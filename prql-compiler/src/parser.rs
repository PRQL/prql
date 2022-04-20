//! This module contains the parser, which is responsible for converting a tree
//! of pest pairs into a tree of AST Items. It has a small function to call into
//! pest to get the parse tree / concrete syntaxt tree, and then a large
//! function for turning that into PRQL AST.
use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

use super::ast::*;
use super::utils::*;
use crate::error::{Error, Reason, Span};

#[derive(Parser)]
#[grammar = "prql.pest"]
struct PrqlParser;

pub(crate) type PestError = pest::error::Error<Rule>;
pub(crate) type PestRule = Rule;

/// Build an AST from a PRQL query string.
pub fn parse(string: &str) -> Result<Query> {
    let ast = ast_of_string(string, Rule::query)?;

    ast.item.into_query().map_err(|_| unreachable!())
}

/// Parse a string into an AST. Unlike [parse], this can start from any rule.
fn ast_of_string(string: &str, rule: Rule) -> Result<Node> {
    let pairs = parse_tree_of_str(string, rule)?;

    ast_of_parse_tree(pairs)?.into_only()
}

/// Parse a string into a parse tree / concrete syntax tree, made up of pest Pairs.
fn parse_tree_of_str(source: &str, rule: Rule) -> Result<Pairs<Rule>> {
    Ok(PrqlParser::parse(rule, source)?)
}

/// Parses a parse tree of pest Pairs into an AST.
fn ast_of_parse_tree(pairs: Pairs<Rule>) -> Result<Vec<Node>> {
    pairs
        // Exclude end-of-input at the moment.
        .filter(|pair| pair.as_rule() != Rule::EOI)
        .map(|pair| {
            let span = pair.as_span();

            let item = match pair.as_rule() {
                Rule::query => {
                    let mut parsed = ast_of_parse_tree(pair.into_inner())?;
                    // this is [query, ...]

                    let mut query = parsed.remove(0).item.into_query()?;

                    query.nodes = parsed;

                    Item::Query(query)
                }
                Rule::query_def => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;

                    let mut params: HashMap<_, _> = parsed
                        .into_iter()
                        .map(|x| x.item.into_named_expr().map(|n| (n.name, n.expr)))
                        .try_collect()?;

                    let version = params
                        .remove("version")
                        .map(|v| v.unwrap(|i| i.into_ident(), "string"))
                        .transpose()?;

                    let dialect = if let Some(node) = params.remove("dialect") {
                        let span = node.span;
                        let dialect = node.unwrap(|i| i.into_ident(), "string")?;
                        Dialect::from_str(&dialect).map_err(|_| {
                            Error::new(Reason::NotFound {
                                name: dialect,
                                namespace: "dialect".to_string(),
                            })
                            .with_span(span)
                        })?
                    } else {
                        Dialect::default()
                    };

                    Item::Query(Query {
                        nodes: vec![],
                        version,
                        dialect,
                    })
                }
                Rule::list => Item::List(
                    ast_of_parse_tree(pair.into_inner())?
                        .into_iter()
                        .map(ListItem)
                        .collect(),
                ),
                Rule::expr_mul
                | Rule::expr_add
                | Rule::expr_compare
                | Rule::expr
                | Rule::expr_call => ast_of_parse_tree(pair.into_inner())?.into_expr(),

                Rule::named_expr_call | Rule::named_expr | Rule::named_term => {
                    let items = ast_of_parse_tree(pair.into_inner())?;
                    // this borrow could be removed, but it becomes much less readable without match
                    match &items[..] {
                        [Node {
                            item: Item::Ident(name),
                            ..
                        }, node] => Item::NamedExpr(NamedExpr {
                            name: name.clone(),
                            expr: Box::new(node.clone()),
                        }),
                        [node] => node.item.clone(),
                        _ => unreachable!(),
                    }
                }
                Rule::func_def => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;

                    let [flags, name, params, body]: [Node; 4] = parsed
                        .try_into()
                        .map_err(|_| anyhow!("bad func_def parsing"))?;

                    let kind = FuncKind::from_str(&flags.item.into_raw()?).ok();
                    let name = name.item.into_ident()?;
                    let params = params.item.into_expr()?;

                    let positional_params = params
                        .iter()
                        .filter(|x| matches!(x.item, Item::Ident(_)))
                        .cloned()
                        .collect();
                    let named_params = params
                        .iter()
                        .filter(|x| matches!(x.item, Item::NamedExpr(_)))
                        .cloned()
                        .collect();

                    Item::FuncDef(FuncDef {
                        name,
                        kind,
                        positional_params,
                        named_params,
                        body: Box::from(body),
                    })
                }
                Rule::func_def_params => Item::Expr(ast_of_parse_tree(pair.into_inner())?),
                Rule::func_call | Rule::func_curry => {
                    let mut items = ast_of_parse_tree(pair.into_inner())?;

                    let name = items.remove(0).item.into_ident()?;

                    Item::FuncCall(FuncCall {
                        name,
                        args: items,
                        named_args: HashMap::new(),
                    })
                }
                Rule::table => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;
                    let [name, pipeline]: [Node; 2] = parsed
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {e:?}"))?;
                    Item::Table(Table {
                        name: name.item.into_ident()?,
                        pipeline: Box::new(pipeline),
                    })
                }
                Rule::ident => Item::Ident(pair.as_str().to_string()),
                Rule::string_literal => {
                    ast_of_parse_tree(pair.clone().into_inner())?
                        .into_only()?
                        .item
                }
                Rule::string => Item::String(pair.as_str().to_string()),
                Rule::s_string => Item::SString(ast_of_interpolate_items(pair)?),
                Rule::f_string => Item::FString(ast_of_interpolate_items(pair)?),
                Rule::pipeline => Item::Pipeline(Pipeline {
                    value: None,
                    functions: ast_of_parse_tree(pair.into_inner())?,
                }),
                Rule::parenthesized_pipeline => {
                    let mut parsed = ast_of_parse_tree(pair.into_inner())?;
                    // this is either [expr, pipeline] or [pipeline]

                    let first = parsed.remove(0);

                    Item::Pipeline(if let Item::Pipeline(pipeline) = first.item {
                        // no value
                        pipeline
                    } else {
                        // prepend value
                        let mut pipeline = parsed.remove(0).item.into_pipeline()?;

                        pipeline.value = Some(Box::new(first));
                        pipeline
                    })
                }
                Rule::range => {
                    // a bit hacky, but eh
                    let no_start = &pair.as_span().as_str()[0..2] == "..";

                    let mut parsed = ast_of_parse_tree(pair.into_inner())?;

                    let (start, end) = match parsed.len() {
                        0 => (None, None),
                        1 => {
                            let item = Box::from(parsed.remove(0));
                            if no_start {
                                (None, Some(item))
                            } else {
                                (Some(item), None)
                            }
                        }
                        2 => (
                            Some(Box::from(parsed.remove(0))),
                            Some(Box::from(parsed.remove(0))),
                        ),
                        _ => unreachable!(),
                    };
                    Item::Range(Range { start, end })
                }
                Rule::interval => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;
                    // unimplemented!();
                    let [n, unit]: [Node; 2] = parsed
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {e:?}"))?;

                    Item::Interval(Interval {
                        n: n.item.as_raw().unwrap().parse()?,
                        unit: unit.item.as_raw().unwrap().clone(),
                    })
                }
                Rule::operator_unary
                | Rule::operator_mul
                | Rule::operator_add
                | Rule::operator_compare
                | Rule::operator_logical
                | Rule::number
                | Rule::interval_kind
                | Rule::func_def_flags => Item::Raw(pair.as_str().to_owned()),
                _ => unreachable!(),
            };

            let mut node = Node::from(item);
            node.span = Some(Span {
                start: span.start(),
                end: span.end(),
            });
            Ok(node)
        })
        .collect()
}

fn ast_of_interpolate_items(pair: Pair<Rule>) -> Result<Vec<InterpolateItem>> {
    pair.into_inner()
        .map(|x| {
            Ok(match x.as_rule() {
                Rule::interpolate_string => InterpolateItem::String(x.as_str().to_string()),
                _ => InterpolateItem::Expr(ast_of_parse_tree(x.into_inner())?.into_expr().into()),
            })
        })
        .collect::<Result<_>>()
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    #[test]
    fn test_parse_take() {
        parse_tree_of_str("take 10", Rule::query).unwrap();
    }

    #[test]
    fn test_parse_string() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"" U S A ""#, Rule::string_literal)?, @r###"
        [
            Pair {
                rule: string_literal,
                span: Span {
                    str: "\" U S A \"",
                    start: 0,
                    end: 9,
                },
                inner: [
                    Pair {
                        rule: string,
                        span: Span {
                            str: " U S A ",
                            start: 1,
                            end: 8,
                        },
                        inner: [],
                    },
                ],
            },
        ]
        "###);
        let double_quoted_ast = ast_of_string(r#"" U S A ""#, Rule::string_literal)?;
        assert_yaml_snapshot!(double_quoted_ast, @r###"
        ---
        String: " U S A "
        "###);

        let single_quoted_ast = ast_of_string(r#"' U S A '"#, Rule::string_literal)?;
        assert_eq!(single_quoted_ast, double_quoted_ast);

        // Single quotes within double quotes should produce a string containing
        // the single quotes (and vice versa).
        assert_yaml_snapshot!(ast_of_string(r#""' U S A '""#, Rule::string_literal)? , @r###"
        ---
        String: "' U S A '"
        "###);
        assert_yaml_snapshot!(ast_of_string(r#"'" U S A "'"#, Rule::string_literal)? , @r###"
        ---
        String: "\" U S A \""
        "###);

        assert!(ast_of_string(r#"" U S A"#, Rule::string_literal).is_err());
        assert!(ast_of_string(r#"" U S A '"#, Rule::string_literal).is_err());

        // Escapes get passed through (the insta snapshot has them escaped I
        // think, which isn't that clear, so repeated below).
        let escaped_string = ast_of_string(r#"" \U S A ""#, Rule::string_literal)?;
        assert_yaml_snapshot!(escaped_string, @r###"
        ---
        String: " \\U S A "
        "###);
        assert_eq!(escaped_string.item.as_string().unwrap(), r#" \U S A "#);

        // Currently we don't allow escaping closing quotes — because it's not
        // trivial to do in pest, and I'm not sure it's a great idea either — we
        // should arguably encourage r-strings. (Though no objection if someone
        // wants to implement it, this test is recording current behavior rather
        // than maintaining a contract).
        let escaped_quotes = ast_of_string(r#"" Canada \""#, Rule::string_literal)?;
        assert_yaml_snapshot!(escaped_quotes, @r###"
        ---
        String: " Canada \\"
        "###);
        assert_eq!(escaped_quotes.item.as_string().unwrap(), r#" Canada \"#);

        Ok(())
    }

    #[test]
    fn test_parse_s_string() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"s"SUM({col})""#, Rule::expr_call)?);
        assert_yaml_snapshot!(ast_of_string(r#"s"SUM({col})""#, Rule::expr_call)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Ident: col
          - String: )
        "###);
        assert_yaml_snapshot!(ast_of_string(r#"s"SUM({2 + 2})""#, Rule::expr_call)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Expr:
                - Raw: "2"
                - Raw: +
                - Raw: "2"
          - String: )
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_list() {
        assert_debug_snapshot!(parse_tree_of_str(r#"[1 + 1, 2]"#, Rule::list).unwrap());
        assert_yaml_snapshot!(ast_of_string(r#"[1 + 1, 2]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Expr:
              - Raw: "1"
              - Raw: +
              - Raw: "1"
          - Raw: "2"
        "###);
        assert_yaml_snapshot!(ast_of_string(r#"[1 + (f 1), 2]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Expr:
              - Raw: "1"
              - Raw: +
              - FuncCall:
                  name: f
                  args:
                    - Raw: "1"
                  named_args: {}
          - Raw: "2"
        "###);
        // Line breaks
        assert_yaml_snapshot!(ast_of_string(
            r#"[1,

                2]"#,
         Rule::list).unwrap(), @r###"
        ---
        List:
          - Raw: "1"
          - Raw: "2"
        "###);
        // Function call in a list
        let ab = ast_of_string(r#"[a b]"#, Rule::list).unwrap();
        let a_comma_b = ast_of_string(r#"[a, b]"#, Rule::list).unwrap();
        assert_yaml_snapshot!(ab, @r###"
        ---
        List:
          - FuncCall:
              name: a
              args:
                - Ident: b
              named_args: {}
        "###);
        assert_yaml_snapshot!(a_comma_b, @r###"
        ---
        List:
          - Ident: a
          - Ident: b
        "###);
        assert_ne!(ab, a_comma_b);
    }

    #[test]
    fn test_parse_number() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"23"#, Rule::number)?, @r###"
        [
            Pair {
                rule: number,
                span: Span {
                    str: "23",
                    start: 0,
                    end: 2,
                },
                inner: [],
            },
        ]
        "###);
        assert_debug_snapshot!(parse_tree_of_str(r#"2 + 2"#, Rule::expr)?);
        Ok(())
    }

    #[test]
    fn test_parse_filter() {
        assert_yaml_snapshot!(
            ast_of_string(r#"filter country = "USA""#, Rule::query).unwrap(), @r###"
        ---
        Query:
          version: ~
          dialect: Generic
          nodes:
            - Pipeline:
                value: ~
                functions:
                  - FuncCall:
                      name: filter
                      args:
                        - Expr:
                            - Ident: country
                            - Raw: "="
                            - String: USA
                      named_args: {}
        "###);

        assert_yaml_snapshot!(
            ast_of_string(r#"filter (upper country) = "USA""#, Rule::query).unwrap(), @r###"
        ---
        Query:
          version: ~
          dialect: Generic
          nodes:
            - Pipeline:
                value: ~
                functions:
                  - FuncCall:
                      name: filter
                      args:
                        - Expr:
                            - FuncCall:
                                name: upper
                                args:
                                  - Ident: country
                                named_args: {}
                            - Raw: "="
                            - String: USA
                      named_args: {}
        "###
        );
    }

    #[test]
    fn test_parse_aggregate() {
        let aggregate = ast_of_string(
            r"group [title] (
            aggregate [sum salary, count]
        )",
            Rule::pipeline,
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        Pipeline:
          value: ~
          functions:
            - FuncCall:
                name: group
                args:
                  - List:
                      - Ident: title
                  - Pipeline:
                      value: ~
                      functions:
                        - FuncCall:
                            name: aggregate
                            args:
                              - List:
                                  - FuncCall:
                                      name: sum
                                      args:
                                        - Ident: salary
                                      named_args: {}
                                  - Ident: count
                            named_args: {}
                named_args: {}
        "###);
        let aggregate = ast_of_string(
            r"group [title] (
            aggregate [sum salary]
        )",
            Rule::pipeline,
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        Pipeline:
          value: ~
          functions:
            - FuncCall:
                name: group
                args:
                  - List:
                      - Ident: title
                  - Pipeline:
                      value: ~
                      functions:
                        - FuncCall:
                            name: aggregate
                            args:
                              - List:
                                  - FuncCall:
                                      name: sum
                                      args:
                                        - Ident: salary
                                      named_args: {}
                            named_args: {}
                named_args: {}
        "###);
    }

    #[test]
    fn test_parse_select() -> Result<()> {
        assert_yaml_snapshot!(
            ast_of_string(r#"select x"#, Rule::func_curry)?
        , @r###"
        ---
        FuncCall:
          name: select
          args:
            - Ident: x
          named_args: {}
        "###);

        assert_yaml_snapshot!(
            ast_of_string(r#"select [x, y]"#, Rule::func_curry)?
        , @r###"
        ---
        FuncCall:
          name: select
          args:
            - List:
                - Ident: x
                - Ident: y
          named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_expr() -> Result<()> {
        assert_yaml_snapshot!(
            ast_of_string(r#"country = "USA""#, Rule::expr)?
        , @r###"
        ---
        Expr:
          - Ident: country
          - Raw: "="
          - String: USA
        "###);
        assert_yaml_snapshot!(ast_of_string(
                r#"[
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost,
]"#,
        Rule::list)?, @r###"
        ---
        List:
          - NamedExpr:
              name: gross_salary
              expr:
                Expr:
                  - Ident: salary
                  - Raw: +
                  - Ident: payroll_tax
          - NamedExpr:
              name: gross_cost
              expr:
                Expr:
                  - Ident: gross_salary
                  - Raw: +
                  - Ident: benefits_cost
        "###);
        assert_yaml_snapshot!(
            ast_of_string(
                "gross_salary: (salary + payroll_tax) * (1 + tax_rate)",
                Rule::named_expr,
            )?,
            @r###"
        ---
        NamedExpr:
          name: gross_salary
          expr:
            Expr:
              - Expr:
                  - Ident: salary
                  - Raw: +
                  - Ident: payroll_tax
              - Raw: "*"
              - Expr:
                  - Raw: "1"
                  - Raw: +
                  - Ident: tax_rate
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_query() -> Result<()> {
        assert_yaml_snapshot!(ast_of_string(
            r#"
from employees
filter country = "USA"                        # Each line transforms the previous result.
derive [                                      # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
aggregate [               # `by` are the columns to group by.
                   average salary,            # These are aggregation calcs run on each group.
                   sum salary,
                   average gross_salary,
                   sum gross_salary,
                   average gross_cost,
  sum_gross_cost: sum gross_cost,
  ct            : count,
] )
sort sum_gross_cost
filter ct > 200
take 20
    "#
            .trim(),
            Rule::query,
        )?);
        Ok(())
    }

    #[test]
    fn test_parse_function() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(
            "func plus_one x = x + 1",
            Rule::func_def
        )?);
        assert_yaml_snapshot!(ast_of_string(
            "func identity x = x", Rule::func_def
        )?
        , @r###"
        ---
        FuncDef:
          name: identity
          kind: ~
          positional_params:
            - Ident: x
          named_params: []
          body:
            Ident: x
        "###);
        assert_yaml_snapshot!(ast_of_string(
            "func plus_one x = (x + 1)", Rule::func_def
        )?
        , @r###"
        ---
        FuncDef:
          name: plus_one
          kind: ~
          positional_params:
            - Ident: x
          named_params: []
          body:
            Expr:
              - Ident: x
              - Raw: +
              - Raw: "1"
        "###);
        assert_yaml_snapshot!(ast_of_string(
            "func plus_one x = x + 1", Rule::func_def
        )?
        , @r###"
        ---
        FuncDef:
          name: plus_one
          kind: ~
          positional_params:
            - Ident: x
          named_params: []
          body:
            Expr:
              - Ident: x
              - Raw: +
              - Raw: "1"
        "###);
        // An example to show that we can't delayer the tree, despite there
        // being lots of layers.
        assert_yaml_snapshot!(ast_of_string(
            "func foo x = (foo bar + 1) (plax) - baz", Rule::func_def
        )?
        , @r###"
        ---
        FuncDef:
          name: foo
          kind: ~
          positional_params:
            - Ident: x
          named_params: []
          body:
            FuncCall:
              name: foo
              args:
                - Expr:
                    - Ident: bar
                    - Raw: +
                    - Raw: "1"
              named_args: {}
        "###);

        assert_yaml_snapshot!(ast_of_string("func return_constant = 42", Rule::func_def)?, @r###"
        ---
        FuncDef:
          name: return_constant
          kind: ~
          positional_params: []
          named_params: []
          body:
            Raw: "42"
        "###);
        assert_yaml_snapshot!(ast_of_string(r#"func count X = s"SUM({X})""#, Rule::func_def)?, @r###"
        ---
        FuncDef:
          name: count
          kind: ~
          positional_params:
            - Ident: X
          named_params: []
          body:
            SString:
              - String: SUM(
              - Expr:
                  Ident: X
              - String: )
        "###);

        /* TODO: Does not yet parse because `window` not yet implemented.
            assert_debug_snapshot!(ast_of_parse_tree(
                parse_tree_of_str(
                    r#"
        func lag_day x = (
          window x
          by sec_id
          sort date
          lag 1
        )
                    "#,
                    Rule::func_def
                )
                .unwrap()
            ));
            */

        assert_yaml_snapshot!(ast_of_string(r#"func add x to:a = x + to"#, Rule::func_def)?, @r###"
        ---
        FuncDef:
          name: add
          kind: ~
          positional_params:
            - Ident: x
          named_params:
            - NamedExpr:
                name: to
                expr:
                  Ident: a
          body:
            Expr:
              - Ident: x
              - Raw: +
              - Ident: to
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_func_call() {
        // Function without argument
        let ast = ast_of_string(r#"count"#, Rule::expr).unwrap();
        let ident = ast.item.into_ident().unwrap();
        assert_yaml_snapshot!(
            ident, @r###"
        ---
        count
        "###);

        // A non-friendly option for #154
        let ast = ast_of_string(r#"count s'*'"#, Rule::expr_call).unwrap();
        let func_call: FuncCall = ast.item.into_func_call().unwrap();
        assert_yaml_snapshot!(
            func_call, @r###"
        ---
        name: count
        args:
          - SString:
              - String: "*"
        named_args: {}
        "###);

        assert_yaml_snapshot!(parse(r#"from mytable | select [a and b + c or (d e) and f]"#).unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: mytable
                    named_args: {}
                - FuncCall:
                    name: select
                    args:
                      - List:
                          - Expr:
                              - Ident: a
                              - Raw: and
                              - Expr:
                                  - Ident: b
                                  - Raw: +
                                  - Ident: c
                              - Raw: or
                              - FuncCall:
                                  name: d
                                  args:
                                    - Ident: e
                                  named_args: {}
                              - Raw: and
                              - Ident: f
                    named_args: {}
        "###);

        let ast = ast_of_string(r#"add bar to:3"#, Rule::expr_call).unwrap();
        assert_yaml_snapshot!(
            ast, @r###"
        ---
        FuncCall:
          name: add
          args:
            - Ident: bar
            - NamedExpr:
                name: to
                expr:
                  Raw: "3"
          named_args: {}
        "###);
    }

    #[test]
    fn test_parse_table() -> Result<()> {
        assert_yaml_snapshot!(ast_of_string(
            "table newest_employees = (| from employees )",
            Rule::table
        )?, @r###"
        ---
        Table:
          name: newest_employees
          pipeline:
            Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: employees
                    named_args: {}
        "###);

        assert_yaml_snapshot!(ast_of_string(
            r#"
        table newest_employees = (
          from employees
          group country (
            aggregate [
                average_country_salary: average salary
            ]
          )
          sort tenure
          take 50
        )"#.trim(), Rule::table)?,
         @r###"
        ---
        Table:
          name: newest_employees
          pipeline:
            Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: employees
                    named_args: {}
                - FuncCall:
                    name: group
                    args:
                      - Ident: country
                      - Pipeline:
                          value: ~
                          functions:
                            - FuncCall:
                                name: aggregate
                                args:
                                  - List:
                                      - NamedExpr:
                                          name: average_country_salary
                                          expr:
                                            FuncCall:
                                              name: average
                                              args:
                                                - Ident: salary
                                              named_args: {}
                                named_args: {}
                    named_args: {}
                - FuncCall:
                    name: sort
                    args:
                      - Ident: tenure
                    named_args: {}
                - FuncCall:
                    name: take
                    args:
                      - Raw: "50"
                    named_args: {}
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_into_parse_tree() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"country = "USA""#, Rule::expr)?);
        assert_debug_snapshot!(parse_tree_of_str("select [a, b, c]", Rule::func_curry)?);
        assert_debug_snapshot!(parse_tree_of_str(
            "group [title, country] (
                aggregate [sum salary]
            )",
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"    filter country = "USA""#,
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str("[a, b, c,]", Rule::list)?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"[
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost
]"#,
            Rule::list
        )?);
        // Currently not putting comments in our parse tree, so this is blank.
        assert_debug_snapshot!(parse_tree_of_str(
            r#"# this is a comment
        select a"#,
            Rule::COMMENT
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            "join country [id=employee_id]",
            Rule::func_curry
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            "join side:left country [id=employee_id]",
            Rule::func_curry
        )?);
        assert_debug_snapshot!(parse_tree_of_str("1  + 2", Rule::expr)?);
        Ok(())
    }

    #[test]
    fn test_inline_pipeline() {
        assert_debug_snapshot!(parse_tree_of_str(
            "(salary | percentile 50)",
            Rule::parenthesized_pipeline
        )
        .unwrap());
        assert_yaml_snapshot!(ast_of_string("(salary | percentile 50)", Rule::parenthesized_pipeline).unwrap(), @r###"
        ---
        Pipeline:
          value:
            Ident: salary
          functions:
            - FuncCall:
                name: percentile
                args:
                  - Raw: "50"
                named_args: {}
        "###);
        assert_yaml_snapshot!(ast_of_string("func median x = (x | percentile 50)", Rule::query).unwrap(), @r###"
        ---
        Query:
          version: ~
          dialect: Generic
          nodes:
            - FuncDef:
                name: median
                kind: ~
                positional_params:
                  - Ident: x
                named_params: []
                body:
                  Pipeline:
                    value:
                      Ident: x
                    functions:
                      - FuncCall:
                          name: percentile
                          args:
                            - Raw: "50"
                          named_args: {}
        "###);
    }

    #[test]
    fn test_parse_pipeline_parse_tree() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(
            r#"
    from employees
    select [a, b]
    "#
            .trim(),
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"
    from employees
    filter country = "USA"
    "#
            .trim(),
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate [                                  # `by` are the columns to group by.
        average salary,                          # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost: sum gross_cost,
        count: count,
    ]
)
sort sum_gross_cost
filter count > 200
take 20
    "#
            .trim(),
            Rule::pipeline
        )?);
        Ok(())
    }

    #[test]
    fn test_parse_sql_parameters() -> Result<()> {
        assert_yaml_snapshot!(parse(r#"
        from mytable
        filter [
            first_name = $1,
            last_name = $2.name
        ]
        "#)?, @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: mytable
                    named_args: {}
                - FuncCall:
                    name: filter
                    args:
                      - List:
                          - Expr:
                              - Ident: first_name
                              - Raw: "="
                              - Ident: $1
                          - Expr:
                              - Ident: last_name
                              - Raw: "="
                              - Ident: $2.name
                    named_args: {}
        "###);
        Ok(())
    }

    #[test]
    fn test_tab_characters() -> Result<()> {
        // #284

        let prql = "from c_invoice
join doc:c_doctype [c_invoice_id]
select [
\tinvoice_no,
\tdocstatus
]";
        parse(prql)?;

        Ok(())
    }

    #[test]
    fn test_sort() {
        assert_yaml_snapshot!(parse("
        from invoices
        sort issued_at
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: invoices
                    named_args: {}
                - FuncCall:
                    name: sort
                    args:
                      - Ident: issued_at
                    named_args: {}
        "###);

        assert_yaml_snapshot!(parse("
        from invoices
        sort [desc:issued_at]
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: invoices
                    named_args: {}
                - FuncCall:
                    name: sort
                    args:
                      - List:
                          - NamedExpr:
                              name: desc
                              expr:
                                Ident: issued_at
                    named_args: {}
        "###);

        assert_yaml_snapshot!(parse("
        from invoices
        sort [asc:issued_at]
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: invoices
                    named_args: {}
                - FuncCall:
                    name: sort
                    args:
                      - List:
                          - NamedExpr:
                              name: asc
                              expr:
                                Ident: issued_at
                    named_args: {}
        "###);

        assert_yaml_snapshot!(parse("
        from invoices
        sort [asc:issued_at, desc:amount, num_of_articles]
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: invoices
                    named_args: {}
                - FuncCall:
                    name: sort
                    args:
                      - List:
                          - NamedExpr:
                              name: asc
                              expr:
                                Ident: issued_at
                          - NamedExpr:
                              name: desc
                              expr:
                                Ident: amount
                          - Ident: num_of_articles
                    named_args: {}
        "###);
    }

    #[test]
    fn test_range() {
        assert_yaml_snapshot!(parse("
        from employees
        filter (age | between 18..40)
        derive [greater_than_ten: 11..]
        derive [less_than_ten: ..9]
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: employees
                    named_args: {}
                - FuncCall:
                    name: filter
                    args:
                      - Pipeline:
                          value:
                            Ident: age
                          functions:
                            - FuncCall:
                                name: between
                                args:
                                  - Range:
                                      start:
                                        Raw: "18"
                                      end:
                                        Raw: "40"
                                named_args: {}
                    named_args: {}
                - FuncCall:
                    name: derive
                    args:
                      - List:
                          - NamedExpr:
                              name: greater_than_ten
                              expr:
                                Range:
                                  start:
                                    Raw: "11"
                                  end: ~
                    named_args: {}
                - FuncCall:
                    name: derive
                    args:
                      - List:
                          - NamedExpr:
                              name: less_than_ten
                              expr:
                                Range:
                                  start: ~
                                  end:
                                    Raw: "9"
                    named_args: {}
        "###);
    }

    #[test]
    fn test_interval() {
        assert_yaml_snapshot!(parse("
        from employees
        derive [age_plus_two_years: (age + 2years)]
        ").unwrap(), @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
          - Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                      - Ident: employees
                    named_args: {}
                - FuncCall:
                    name: derive
                    args:
                      - List:
                          - NamedExpr:
                              name: age_plus_two_years
                              expr:
                                Expr:
                                  - Ident: age
                                  - Raw: +
                                  - Interval:
                                      n: 2
                                      unit: years
                    named_args: {}
        "###);
    }
}
