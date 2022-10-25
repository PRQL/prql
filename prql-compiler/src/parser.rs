//! This module contains the parser, which is responsible for converting a tree
//! of pest pairs into a tree of AST Items. It has a small function to call into
//! pest to get the parse tree / concrete syntax tree, and then a large
//! function for turning that into PRQL AST.
use std::collections::HashMap;
use std::str::FromStr;

use anyhow::bail;
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
pub fn parse(string: &str) -> Result<Vec<Stmt>> {
    let pairs = parse_tree_of_str(string, Rule::statements)?;

    stmts_of_parse_pairs(pairs)
}

/// Parse a string into a parse tree / concrete syntax tree, made up of pest Pairs.
fn parse_tree_of_str(source: &str, rule: Rule) -> Result<Pairs<Rule>> {
    Ok(PrqlParser::parse(rule, source)?)
}

/// Parses a parse tree of pest Pairs into a list of statements.
fn stmts_of_parse_pairs(pairs: Pairs<Rule>) -> Result<Vec<Stmt>> {
    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(stmt_of_parse_pair)
        .collect()
}

fn stmt_of_parse_pair(pair: Pair<Rule>) -> Result<Stmt> {
    let span = pair.as_span();
    let rule = pair.as_rule();

    let kind = match rule {
        Rule::pipeline_stmt => {
            let pipeline = expr_of_parse_pair(pair.into_inner().next().unwrap())?;
            StmtKind::Pipeline(pipeline)
        }
        Rule::query_def => {
            let mut params: HashMap<_, _> = pair
                .into_inner()
                .map(|x| exprs_of_parse_pairs(x.into_inner()).map(parse_named))
                .try_collect()?;

            let version = params
                .remove("version")
                .map(|v| v.try_cast(|i| i.parse_version(), None, "semver version number string"))
                .transpose()?;

            let dialect = if let Some(node) = params.remove("dialect") {
                let span = node.span;
                let dialect = node.try_cast(|i| i.into_ident(), None, "string")?;
                Dialect::from_str(&dialect.to_string()).map_err(|_| {
                    Error::new(Reason::NotFound {
                        name: dialect.to_string(),
                        namespace: "dialect".to_string(),
                    })
                    .with_span(span)
                })?
            } else {
                Dialect::default()
            };

            StmtKind::QueryDef(QueryDef { version, dialect })
        }
        Rule::func_def => {
            let mut pairs = pair.into_inner();
            let name = pairs.next().unwrap();
            let params = pairs.next().unwrap();
            let body = pairs.next().unwrap();

            let (name, return_type, _) = parse_typed_ident(name)?;

            let params: Vec<_> = params
                .into_inner()
                .into_iter()
                .map(parse_typed_ident)
                .try_collect()?;

            let mut positional_params = vec![];
            let mut named_params = vec![];
            for (name, ty, default_value) in params {
                let param = FuncParam {
                    name,
                    ty,
                    default_value,
                };
                if param.default_value.is_some() {
                    named_params.push(param)
                } else {
                    positional_params.push(param)
                }
            }

            StmtKind::FuncDef(FuncDef {
                name,
                positional_params,
                named_params,
                body: Box::from(expr_of_parse_pair(body)?),
                return_ty: return_type,
            })
        }
        Rule::table_def => {
            let parsed = exprs_of_parse_pairs(pair.into_inner())?;
            let [name, pipeline]: [Expr; 2] = parsed
                .try_into()
                .map_err(|e| anyhow!("Expected two items; {e:?}"))?;
            StmtKind::TableDef(TableDef {
                id: None,
                name: name.kind.into_ident()?.to_string(),
                value: Box::new(pipeline),
            })
        }
        _ => unreachable!("{pair}"),
    };
    let mut stmt = Stmt::from(kind);
    stmt.span = Some(Span {
        start: span.start(),
        end: span.end(),
    });
    Ok(stmt)
}

/// Parses a parse tree of pest Pairs into an AST.
fn exprs_of_parse_pairs(pairs: Pairs<Rule>) -> Result<Vec<Expr>> {
    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(expr_of_parse_pair)
        .collect()
}

fn expr_of_parse_pair(pair: Pair<Rule>) -> Result<Expr> {
    let span = pair.as_span();
    let rule = pair.as_rule();
    let mut alias = None;

    let kind = match rule {
        Rule::list => ExprKind::List(exprs_of_parse_pairs(pair.into_inner())?),
        Rule::expr_mul | Rule::expr_add | Rule::expr_compare | Rule::expr => {
            let mut pairs = pair.into_inner();

            let mut expr = expr_of_parse_pair(pairs.next().unwrap())?;
            if let Some(op) = pairs.next() {
                let op = BinOp::from_str(op.as_str())?;

                expr = Expr::from(ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(expr_of_parse_pair(pairs.next().unwrap())?),
                });
            }

            expr.kind
        }
        Rule::expr_unary => {
            let mut pairs = pair.into_inner();

            let op = pairs.next().unwrap();

            let a = expr_of_parse_pair(pairs.next().unwrap())?;
            match UnOp::from_str(op.as_str()) {
                Ok(op) => ExprKind::Unary {
                    op,
                    expr: Box::new(a),
                },
                Err(_) => a.kind, // `+column` is the same as `column`
            }
        }

        // With coalesce, we need to grab the left and the right,
        // because we're transforming it into a function call rather
        // than passing along the operator. So this is unlike the rest
        // of the parsing (and maybe isn't optimal).
        Rule::expr_coalesce => {
            let pairs = pair.into_inner();
            // If there's no coalescing, just return the single expression.
            if pairs.clone().count() == 1 {
                exprs_of_parse_pairs(pairs)?.into_only()?.kind
            } else {
                let parsed = exprs_of_parse_pairs(pairs)?;
                ExprKind::FuncCall(FuncCall {
                    name: Box::new(ExprKind::Ident(Ident::new_name("coalesce")).into()),
                    args: vec![parsed[0].clone(), parsed[2].clone()],
                    named_args: HashMap::new(),
                })
            }
        }
        // This makes the previous parsing a bit easier, but is hacky;
        // ideally find a better way (but it doesn't seem that easy to
        // parse parts of a Pairs).
        Rule::operator_coalesce => ExprKind::Ident(Ident::new_name("-")),

        Rule::assign_call | Rule::assign => {
            let (a, expr) = parse_named(exprs_of_parse_pairs(pair.into_inner())?);
            alias = Some(a);
            expr.kind
        }
        Rule::func_call => {
            let mut pairs = pair.into_inner();

            let name = expr_of_parse_pair(pairs.next().unwrap())?;

            let mut named = HashMap::new();
            let mut positional = Vec::new();
            for arg in pairs {
                match arg.as_rule() {
                    Rule::named_arg => {
                        let (a, expr) = parse_named(exprs_of_parse_pairs(arg.into_inner())?);
                        named.insert(a, expr);
                    }
                    _ => {
                        positional.push(expr_of_parse_pair(arg)?);
                    }
                }
            }

            ExprKind::FuncCall(FuncCall {
                name: Box::new(name),
                args: positional,
                named_args: named,
            })
        }
        Rule::jinja => {
            let inner = pair.as_str();
            ExprKind::Ident(Ident::new_name(inner))
        }
        Rule::ident => {
            let inner = pair.clone().into_inner();
            inner
                .into_iter()
                .map(|x| x.as_str().to_string())
                .collect::<Vec<String>>()
                // `name` is the final item (i.e. `bar` in `foo.bar`); `namespace` is all
                // the items before that. So we split the last item off as
                // `name` and only add a `namespace` if there's at least one
                // additional item.
                .split_last()
                .map(|(name, namespace)| {
                    ExprKind::Ident(Ident {
                        namespace: if namespace.is_empty() {
                            None
                        } else {
                            Some(namespace.join("."))
                        },
                        name: name.to_string(),
                    })
                })
                .unwrap()
        }

        Rule::number => {
            let str = pair.as_str();

            let lit = if let Ok(i) = str.parse::<i64>() {
                Literal::Integer(i)
            } else if let Ok(f) = str.parse::<f64>() {
                Literal::Float(f)
            } else {
                bail!("cannot parse {str} as number")
            };
            ExprKind::Literal(lit)
        }
        Rule::null => ExprKind::Literal(Literal::Null),
        Rule::boolean => ExprKind::Literal(Literal::Boolean(pair.as_str() == "true")),
        Rule::string => {
            // Takes the string_inner, without the quotes
            let inner = pair.into_inner().into_only();
            let inner = inner.map(|x| x.as_str().to_string()).unwrap_or_default();
            ExprKind::Literal(Literal::String(inner))
        }
        Rule::s_string => ExprKind::SString(ast_of_interpolate_items(pair)?),
        Rule::f_string => ExprKind::FString(ast_of_interpolate_items(pair)?),
        Rule::pipeline => {
            let mut nodes = exprs_of_parse_pairs(pair.into_inner())?;
            match nodes.len() {
                0 => unreachable!(),
                1 => nodes.remove(0).kind,
                _ => ExprKind::Pipeline(Pipeline { exprs: nodes }),
            }
        }
        Rule::nested_pipeline => {
            if let Some(pipeline) = pair.into_inner().next() {
                expr_of_parse_pair(pipeline)?.kind
            } else {
                ExprKind::Literal(Literal::Null)
            }
        }
        Rule::range => {
            let [start, end]: [Option<Box<Expr>>; 2] = pair
                .into_inner()
                // Iterate over `start` & `end` (separator is not a term).
                .into_iter()
                .map(|x| {
                    // Parse & Box each one.
                    exprs_of_parse_pairs(x.into_inner())
                        .and_then(|x| x.into_only())
                        .map(Box::new)
                        .ok()
                })
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|e| anyhow!("Expected start, separator, end; {e:?}"))?;
            ExprKind::Range(Range { start, end })
        }

        Rule::value_and_unit => {
            let pairs: Vec<_> = pair.into_inner().into_iter().collect();
            let [n, unit]: [Pair<Rule>; 2] = pairs
                .try_into()
                .map_err(|e| anyhow!("Expected two items; {e:?}"))?;

            ExprKind::Literal(Literal::ValueAndUnit(ValueAndUnit {
                n: n.as_str().parse()?,
                unit: unit.as_str().to_owned(),
            }))
        }

        Rule::date | Rule::time | Rule::timestamp => {
            let inner = pair.into_inner().into_only()?.as_str().to_string();

            ExprKind::Literal(match rule {
                Rule::date => Literal::Date(inner),
                Rule::time => Literal::Time(inner),
                Rule::timestamp => Literal::Timestamp(inner),
                _ => unreachable!(),
            })
        }

        _ => unreachable!("{pair}"),
    };
    Ok(Expr {
        kind,
        span: Some(Span {
            start: span.start(),
            end: span.end(),
        }),
        alias,
        declared_at: None,
        ty: None,
        is_complex: false,
    })
}

fn type_of_parse_pair(pair: Pair<Rule>) -> Result<Ty> {
    let any_of_terms: Vec<_> = pair
        .into_inner()
        .into_iter()
        .map(|pair| -> Result<Ty> {
            let mut pairs = pair.into_inner();
            let name = pairs.next().unwrap().as_str();
            let typ = match TyLit::from_str(name) {
                Ok(t) => Ty::from(t),
                Err(_) if name == "table" => Ty::Table(Frame::default()),
                Err(_) => {
                    eprintln!("named type: {}", name);
                    Ty::Named(name.to_string())
                }
            };

            let param = pairs.next().map(type_of_parse_pair).transpose()?;

            Ok(if let Some(param) = param {
                Ty::Parameterized(Box::new(typ), Box::new(param))
            } else {
                typ
            })
        })
        .try_collect()?;

    // is there only a single element?
    Ok(match <[_; 1]>::try_from(any_of_terms) {
        Ok([only]) => only,
        Err(many) => Ty::AnyOf(many),
    })
}

fn parse_typed_ident(pair: Pair<Rule>) -> Result<(String, Option<Ty>, Option<Expr>)> {
    let mut pairs = pair.into_inner();

    let name = pairs.next().unwrap().as_str().to_string();

    let mut ty = None;
    let mut default = None;
    for pair in pairs {
        if matches!(pair.as_rule(), Rule::type_def) {
            ty = Some(type_of_parse_pair(pair)?);
        } else {
            default = Some(expr_of_parse_pair(pair)?);
        }
    }

    Ok((name, ty, default))
}

fn parse_named(mut items: Vec<Expr>) -> (String, Expr) {
    let expr = items.remove(1);
    let alias = items.remove(0).kind.into_ident().unwrap();
    (alias.to_string(), expr)
}

fn ast_of_interpolate_items(pair: Pair<Rule>) -> Result<Vec<InterpolateItem>> {
    pair.into_inner()
        .map(|x| {
            Ok(match x.as_rule() {
                Rule::interpolate_string_inner_literal
                // The double bracket literals are already stripped of their
                // outer brackets by pest, so we pass them through as strings.
                | Rule::interpolate_double_bracket_literal => {
                    InterpolateItem::String(x.as_str().to_string())
                }
                _ => InterpolateItem::Expr(Box::new(expr_of_parse_pair(x)?)),
            })
        })
        .collect::<Result<_>>()
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    fn stmts_of_string(string: &str) -> Result<Vec<Stmt>> {
        let pairs = parse_tree_of_str(string, Rule::statements)?;

        stmts_of_parse_pairs(pairs)
    }

    fn expr_of_string(string: &str, rule: Rule) -> Result<Expr> {
        let mut pairs = parse_tree_of_str(string, rule)?;

        expr_of_parse_pair(pairs.next().unwrap())
    }

    #[test]
    fn test_parse_take() -> Result<()> {
        parse_tree_of_str("take 10", Rule::statements)?;

        assert_yaml_snapshot!(stmts_of_string(r#"take 10"#)?, @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: take
              args:
                - Literal:
                    Integer: 10
              named_args: {}
        "###);

        assert_yaml_snapshot!(stmts_of_string(r#"take 1..10"#)?, @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: take
              args:
                - Range:
                    start:
                      Literal:
                        Integer: 1
                    end:
                      Literal:
                        Integer: 10
              named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_pipeline_parse_tree() {
        assert_debug_snapshot!(parse_tree_of_str(
            // It's useful to have canonical examples rather than copy-pasting
            // everything, so we reference the prql file here. But a downside of
            // this implementation is: if there's an error in extracting the
            // example from the docs into the file specified here, this test
            // won't compile. Because `cargo insta test --accept` on the
            // workspace — which extracts the example — requires compiling this,
            // we can get stuck.
            //
            // Breaking out of that requires running this `cargo insta test
            // --accept` within `book`, and then running it on the workspace.
            // `task test-all` does this.
            //
            // If we change this, it would great if we can retain having
            // examples tested in the docs.
            &include_str!("../../book/tests/prql/examples/variables-0.prql")
                .trim()
                // Required for Windows
                .replace("\r\n", "\n"),
            Rule::pipeline
        )
        .unwrap());
    }
    #[test]
    fn test_parse_into_parse_tree() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"country == "USA""#, Rule::expr)?);
        assert_debug_snapshot!(parse_tree_of_str("select [a, b, c]", Rule::func_call)?);
        assert_debug_snapshot!(parse_tree_of_str(
            "group [title, country] (
                aggregate [sum salary]
            )",
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"    filter country == "USA""#,
            Rule::pipeline
        )?);
        assert_debug_snapshot!(parse_tree_of_str("[a, b, c,]", Rule::list)?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"[
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost
]"#,
            Rule::list
        )?);
        // Currently not putting comments in our parse tree, so this is blank.
        assert_debug_snapshot!(parse_tree_of_str(
            r#"# this is a comment
        select a"#,
            Rule::COMMENT
        )?, @"[]");
        assert_debug_snapshot!(parse_tree_of_str(
            "join side:left country [id==employee_id]",
            Rule::func_call
        )?);
        assert_debug_snapshot!(parse_tree_of_str("1  + 2", Rule::expr)?);
        Ok(())
    }

    #[test]
    fn test_parse_string() -> Result<()> {
        let double_quoted_ast = expr_of_string(r#"" U S A ""#, Rule::string)?;
        assert_yaml_snapshot!(double_quoted_ast, @r###"
        ---
        Literal:
          String: " U S A "
        "###);

        let single_quoted_ast = expr_of_string(r#"' U S A '"#, Rule::string)?;
        assert_eq!(single_quoted_ast, double_quoted_ast);

        // Single quotes within double quotes should produce a string containing
        // the single quotes (and vice versa).
        assert_yaml_snapshot!(expr_of_string(r#""' U S A '""#, Rule::string)? , @r###"
        ---
        Literal:
          String: "' U S A '"
        "###);
        assert_yaml_snapshot!(expr_of_string(r#"'" U S A "'"#, Rule::string)? , @r###"
        ---
        Literal:
          String: "\" U S A \""
        "###);

        assert!(expr_of_string(r#"" U S A"#, Rule::string).is_err());
        assert!(expr_of_string(r#"" U S A '"#, Rule::string).is_err());

        // Escapes get passed through (the insta snapshot has them escaped I
        // think, which isn't that clear, so repeated below).
        let escaped_string = expr_of_string(r#"" \U S A ""#, Rule::string)?;
        assert_yaml_snapshot!(escaped_string, @r###"
        ---
        Literal:
          String: " \\U S A "
        "###);
        assert_eq!(
            escaped_string
                .kind
                .as_literal()
                .unwrap()
                .as_string()
                .unwrap(),
            r#" \U S A "#
        );

        // Currently we don't allow escaping closing quotes — because it's not
        // trivial to do in pest, and I'm not sure it's a great idea either — we
        // should arguably encourage multiline-strings. (Though no objection if
        // someone wants to implement it, this test is recording current
        // behavior rather than maintaining a contract).
        let escaped_quotes = expr_of_string(r#"" Canada \""#, Rule::string)?;
        assert_yaml_snapshot!(escaped_quotes, @r###"
        ---
        Literal:
          String: " Canada \\"
        "###);
        assert_eq!(
            escaped_quotes
                .kind
                .as_literal()
                .unwrap()
                .as_string()
                .unwrap(),
            r#" Canada \"#
        );

        let multi_double = expr_of_string(
            r#""""
''
Canada
"

""""#,
            Rule::string,
        )?;
        assert_yaml_snapshot!(multi_double, @r###"
        ---
        Literal:
          String: "\n''\nCanada\n\"\n\n"
        "###);

        let multi_single = expr_of_string(
            r#"'''
Canada
"
"""

'''"#,
            Rule::string,
        )?;
        assert_yaml_snapshot!(multi_single, @r###"
        ---
        Literal:
          String: "\nCanada\n\"\n\"\"\"\n\n"
        "###);

        assert_yaml_snapshot!(
          expr_of_string("''", Rule::string).unwrap(),
          @r###"
        ---
        Literal:
          String: ""
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_s_string() -> Result<()> {
        assert_yaml_snapshot!(expr_of_string(r#"s"SUM({col})""#, Rule::expr_call)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Ident:
                namespace: ~
                name: col
          - String: )
        "###);
        assert_yaml_snapshot!(expr_of_string(r#"s"SUM({2 + 2})""#, Rule::expr_call)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Binary:
                left:
                  Literal:
                    Integer: 2
                op: Add
                right:
                  Literal:
                    Integer: 2
          - String: )
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_s_string_brackets() -> Result<()> {
        // For crystal variables
        assert_yaml_snapshot!(expr_of_string(r#"s"{{?crystal_var}}""#, Rule::expr_call)?, @r###"
        ---
        SString:
          - String: "{?crystal_var}"
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_jinja() -> Result<()> {
        assert_yaml_snapshot!(stmts_of_string(r#"
        from {{ ref('stg_orders') }}
        aggregate (sum order_id)
        "#)?, @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: "{{ ref('stg_orders') }}"
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: aggregate
                    args:
                      - FuncCall:
                          name:
                            Ident:
                              namespace: ~
                              name: sum
                          args:
                            - Ident:
                                namespace: ~
                                name: order_id
                          named_args: {}
                    named_args: {}
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_list() {
        assert_yaml_snapshot!(expr_of_string(r#"[1 + 1, 2]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                Literal:
                  Integer: 1
          - Literal:
              Integer: 2
        "###);
        assert_yaml_snapshot!(expr_of_string(r#"[1 + (f 1), 2]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                FuncCall:
                  name:
                    Ident:
                      namespace: ~
                      name: f
                  args:
                    - Literal:
                        Integer: 1
                  named_args: {}
          - Literal:
              Integer: 2
        "###);
        // Line breaks
        assert_yaml_snapshot!(expr_of_string(
            r#"[1,

                2]"#,
         Rule::list).unwrap(), @r###"
        ---
        List:
          - Literal:
              Integer: 1
          - Literal:
              Integer: 2
        "###);
        // Function call in a list
        let ab = expr_of_string(r#"[a b]"#, Rule::list).unwrap();
        let a_comma_b = expr_of_string(r#"[a, b]"#, Rule::list).unwrap();
        assert_yaml_snapshot!(ab, @r###"
        ---
        List:
          - FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: a
              args:
                - Ident:
                    namespace: ~
                    name: b
              named_args: {}
        "###);
        assert_yaml_snapshot!(a_comma_b, @r###"
        ---
        List:
          - Ident:
              namespace: ~
              name: a
          - Ident:
              namespace: ~
              name: b
        "###);
        assert_ne!(ab, a_comma_b);

        assert_yaml_snapshot!(expr_of_string(r#"[amount, +amount, -amount]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Ident:
              namespace: ~
              name: amount
          - Ident:
              namespace: ~
              name: amount
          - Unary:
              op: Neg
              expr:
                Ident:
                  namespace: ~
                  name: amount
        "###);
        // Operators in list items
        assert_yaml_snapshot!(expr_of_string(r#"[amount, +amount, -amount]"#, Rule::list).unwrap(), @r###"
        ---
        List:
          - Ident:
              namespace: ~
              name: amount
          - Ident:
              namespace: ~
              name: amount
          - Unary:
              op: Neg
              expr:
                Ident:
                  namespace: ~
                  name: amount
        "###);
    }

    #[test]
    fn test_parse_number() -> Result<()> {
        assert_yaml_snapshot!(expr_of_string(r#"23"#, Rule::number)?, @r###"
        ---
        Literal:
          Integer: 23
        "###);
        assert_yaml_snapshot!(expr_of_string(r#"23.6"#, Rule::number)?, @r###"
        ---
        Literal:
          Float: 23.6
        "###);
        assert_yaml_snapshot!(expr_of_string(r#"2 + 2"#, Rule::expr)?, @r###"
        ---
        Binary:
          left:
            Literal:
              Integer: 2
          op: Add
          right:
            Literal:
              Integer: 2
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_filter() {
        assert_yaml_snapshot!(
            stmts_of_string(r#"filter country == "USA""#).unwrap(), @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: filter
              args:
                - Binary:
                    left:
                      Ident:
                        namespace: ~
                        name: country
                    op: Eq
                    right:
                      Literal:
                        String: USA
              named_args: {}
        "###);

        assert_yaml_snapshot!(
            stmts_of_string(r#"filter (upper country) == "USA""#).unwrap(), @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: filter
              args:
                - Binary:
                    left:
                      FuncCall:
                        name:
                          Ident:
                            namespace: ~
                            name: upper
                        args:
                          - Ident:
                              namespace: ~
                              name: country
                        named_args: {}
                    op: Eq
                    right:
                      Literal:
                        String: USA
              named_args: {}
        "###
        );
    }

    #[test]
    fn test_parse_aggregate() {
        let aggregate = stmts_of_string(
            r"group [title] (
                aggregate [sum salary, count]
              )",
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: group
              args:
                - List:
                    - Ident:
                        namespace: ~
                        name: title
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  namespace: ~
                                  name: sum
                              args:
                                - Ident:
                                    namespace: ~
                                    name: salary
                              named_args: {}
                          - Ident:
                              namespace: ~
                              name: count
                    named_args: {}
              named_args: {}
        "###);
        let aggregate = stmts_of_string(
            r"group [title] (
                aggregate [sum salary]
              )",
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: group
              args:
                - List:
                    - Ident:
                        namespace: ~
                        name: title
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  namespace: ~
                                  name: sum
                              args:
                                - Ident:
                                    namespace: ~
                                    name: salary
                              named_args: {}
                    named_args: {}
              named_args: {}
        "###);
    }

    #[test]
    fn test_parse_derive() -> Result<()> {
        assert_yaml_snapshot!(
            expr_of_string(r#"derive [x = 5, y = (-x)]"#, Rule::func_call)?
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              namespace: ~
              name: derive
          args:
            - List:
                - Literal:
                    Integer: 5
                  alias: x
                - Unary:
                    op: Neg
                    expr:
                      Ident:
                        namespace: ~
                        name: x
                  alias: y
          named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_select() -> Result<()> {
        assert_yaml_snapshot!(
            expr_of_string(r#"select x"#, Rule::func_call)?
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              namespace: ~
              name: select
          args:
            - Ident:
                namespace: ~
                name: x
          named_args: {}
        "###);

        assert_yaml_snapshot!(
            expr_of_string(r#"select [x, y]"#, Rule::func_call)?
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              namespace: ~
              name: select
          args:
            - List:
                - Ident:
                    namespace: ~
                    name: x
                - Ident:
                    namespace: ~
                    name: y
          named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_expr() -> Result<()> {
        assert_yaml_snapshot!(
            expr_of_string(r#"country == "USA""#, Rule::expr)?
        , @r###"
        ---
        Binary:
          left:
            Ident:
              namespace: ~
              name: country
          op: Eq
          right:
            Literal:
              String: USA
        "###);
        assert_yaml_snapshot!(expr_of_string(
                r#"[
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost,
]"#,
        Rule::list)?, @r###"
        ---
        List:
          - Binary:
              left:
                Ident:
                  namespace: ~
                  name: salary
              op: Add
              right:
                Ident:
                  namespace: ~
                  name: payroll_tax
            alias: gross_salary
          - Binary:
              left:
                Ident:
                  namespace: ~
                  name: gross_salary
              op: Add
              right:
                Ident:
                  namespace: ~
                  name: benefits_cost
            alias: gross_cost
        "###);
        assert_yaml_snapshot!(
            expr_of_string(
                "gross_salary = (salary + payroll_tax) * (1 + tax_rate)",
                Rule::assign,
            )?,
            @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Ident:
                  namespace: ~
                  name: salary
              op: Add
              right:
                Ident:
                  namespace: ~
                  name: payroll_tax
          op: Mul
          right:
            Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                Ident:
                  namespace: ~
                  name: tax_rate
        alias: gross_salary
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_query() -> Result<()> {
        assert_yaml_snapshot!(stmts_of_string(
            r#"
from employees
filter country == "USA"                        # Each line transforms the previous result.
derive [                                      # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost =   gross_salary + benefits_cost # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
aggregate [               # `by` are the columns to group by.
                   average salary,            # These are aggregation calcs run on each group.
                   sum salary,
                   average gross_salary,
                   sum gross_salary,
                   average gross_cost,
  sum_gross_cost = sum gross_cost,
  ct             = count,
] )
sort sum_gross_cost
filter ct > 200
take 20
    "#
            .trim()
        )?);
        Ok(())
    }

    #[test]
    fn test_parse_function() -> Result<()> {
        assert_yaml_snapshot!(stmts_of_string("func plus_one x ->  x + 1")?, @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    namespace: ~
                    name: x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);
        assert_yaml_snapshot!(stmts_of_string("func identity x ->  x")?
        , @r###"
        ---
        - FuncDef:
            name: identity
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Ident:
                namespace: ~
                name: x
            return_ty: ~
        "###);
        assert_yaml_snapshot!(stmts_of_string("func plus_one x ->  (x + 1)")?
        , @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    namespace: ~
                    name: x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);
        assert_yaml_snapshot!(stmts_of_string("func plus_one x ->  x + 1")?
        , @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    namespace: ~
                    name: x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);
        assert_yaml_snapshot!(stmts_of_string("func foo x -> some_func (foo bar + 1) (plax) - baz")?
        , @r###"
        ---
        - FuncDef:
            name: foo
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              FuncCall:
                name:
                  Ident:
                    namespace: ~
                    name: some_func
                args:
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: foo
                      args:
                        - Binary:
                            left:
                              Ident:
                                namespace: ~
                                name: bar
                            op: Add
                            right:
                              Literal:
                                Integer: 1
                      named_args: {}
                  - Binary:
                      left:
                        Ident:
                          namespace: ~
                          name: plax
                      op: Sub
                      right:
                        Ident:
                          namespace: ~
                          name: baz
                named_args: {}
            return_ty: ~
        "###);

        assert_yaml_snapshot!(stmts_of_string("func return_constant ->  42")?, @r###"
        ---
        - FuncDef:
            name: return_constant
            positional_params: []
            named_params: []
            body:
              Literal:
                Integer: 42
            return_ty: ~
        "###);
        assert_yaml_snapshot!(stmts_of_string(r#"func count X ->  s"SUM({X})""#)?, @r###"
        ---
        - FuncDef:
            name: count
            positional_params:
              - name: X
                default_value: ~
            named_params: []
            body:
              SString:
                - String: SUM(
                - Expr:
                    Ident:
                      namespace: ~
                      name: X
                - String: )
            return_ty: ~
        "###);

        /* TODO: Does not yet parse because `window` not yet implemented.
            assert_debug_snapshot!(ast_of_parse_tree(
                parse_tree_of_str(
                    r#"
        func lag_day x ->  (
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

        assert_yaml_snapshot!(stmts_of_string(r#"func add x to:a ->  x + to"#)?, @r###"
        ---
        - FuncDef:
            name: add
            positional_params:
              - name: x
                default_value: ~
            named_params:
              - name: to
                default_value:
                  Ident:
                    namespace: ~
                    name: a
            body:
              Binary:
                left:
                  Ident:
                    namespace: ~
                    name: x
                op: Add
                right:
                  Ident:
                    namespace: ~
                    name: to
            return_ty: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_func_call() {
        // Function without argument
        let ast = expr_of_string(r#"count"#, Rule::expr).unwrap();
        let ident = ast.kind.into_ident().unwrap();
        assert_yaml_snapshot!(
            ident, @r###"
        ---
        namespace: ~
        name: count
        "###);

        // A non-friendly option for #154
        let ast = expr_of_string(r#"count s'*'"#, Rule::expr_call).unwrap();
        let func_call: FuncCall = ast.kind.into_func_call().unwrap();
        assert_yaml_snapshot!(
            func_call, @r###"
        ---
        name:
          Ident:
            namespace: ~
            name: count
        args:
          - SString:
              - String: "*"
        named_args: {}
        "###);

        assert_yaml_snapshot!(parse(r#"from mytable | select [a and b + c or (d e) and f]"#).unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: mytable
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: select
                    args:
                      - List:
                          - Binary:
                              left:
                                Ident:
                                  namespace: ~
                                  name: a
                              op: And
                              right:
                                Binary:
                                  left:
                                    Binary:
                                      left:
                                        Ident:
                                          namespace: ~
                                          name: b
                                      op: Add
                                      right:
                                        Ident:
                                          namespace: ~
                                          name: c
                                  op: Or
                                  right:
                                    Binary:
                                      left:
                                        FuncCall:
                                          name:
                                            Ident:
                                              namespace: ~
                                              name: d
                                          args:
                                            - Ident:
                                                namespace: ~
                                                name: e
                                          named_args: {}
                                      op: And
                                      right:
                                        Ident:
                                          namespace: ~
                                          name: f
                    named_args: {}
        "###);

        let ast = expr_of_string(r#"add bar to=3"#, Rule::expr_call).unwrap();
        assert_yaml_snapshot!(
            ast, @r###"
        ---
        FuncCall:
          name:
            Ident:
              namespace: ~
              name: add
          args:
            - Ident:
                namespace: ~
                name: bar
            - Literal:
                Integer: 3
              alias: to
          named_args: {}
        "###);
    }

    #[test]
    fn test_parse_table() -> Result<()> {
        assert_yaml_snapshot!(stmts_of_string(
            "table newest_employees = (from employees)"
        )?, @r###"
        ---
        - TableDef:
            name: newest_employees
            value:
              FuncCall:
                name:
                  Ident:
                    namespace: ~
                    name: from
                args:
                  - Ident:
                      namespace: ~
                      name: employees
                named_args: {}
            id: ~
        "###);

        assert_yaml_snapshot!(stmts_of_string(
            r#"
        table newest_employees = (
          from employees
          group country (
            aggregate [
                average_country_salary = average salary
            ]
          )
          sort tenure
          take 50
        )"#.trim())?,
         @r###"
        ---
        - TableDef:
            name: newest_employees
            value:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: from
                      args:
                        - Ident:
                            namespace: ~
                            name: employees
                      named_args: {}
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: group
                      args:
                        - Ident:
                            namespace: ~
                            name: country
                        - FuncCall:
                            name:
                              Ident:
                                namespace: ~
                                name: aggregate
                            args:
                              - List:
                                  - FuncCall:
                                      name:
                                        Ident:
                                          namespace: ~
                                          name: average
                                      args:
                                        - Ident:
                                            namespace: ~
                                            name: salary
                                      named_args: {}
                                    alias: average_country_salary
                            named_args: {}
                      named_args: {}
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: sort
                      args:
                        - Ident:
                            namespace: ~
                            name: tenure
                      named_args: {}
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: take
                      args:
                        - Literal:
                            Integer: 50
                      named_args: {}
            id: ~
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_table_with_newlines() -> Result<()> {
        assert_yaml_snapshot!(stmts_of_string(
          "table x = (

            from x_table

            select only_in_x = foo

          )

          from x"
        )?, @r###"
        ---
        - TableDef:
            name: x
            value:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: from
                      args:
                        - Ident:
                            namespace: ~
                            name: x_table
                      named_args: {}
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: select
                      args:
                        - Ident:
                            namespace: ~
                            name: foo
                          alias: only_in_x
                      named_args: {}
            id: ~
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: from
              args:
                - Ident:
                    namespace: ~
                    name: x
              named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_inline_pipeline() {
        assert_yaml_snapshot!(expr_of_string("(salary | percentile 50)", Rule::nested_pipeline).unwrap(), @r###"
        ---
        Pipeline:
          exprs:
            - Ident:
                namespace: ~
                name: salary
            - FuncCall:
                name:
                  Ident:
                    namespace: ~
                    name: percentile
                args:
                  - Literal:
                      Integer: 50
                named_args: {}
        "###);
        assert_yaml_snapshot!(stmts_of_string("func median x -> (x | percentile 50)").unwrap(), @r###"
        ---
        - FuncDef:
            name: median
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Pipeline:
                exprs:
                  - Ident:
                      namespace: ~
                      name: x
                  - FuncCall:
                      name:
                        Ident:
                          namespace: ~
                          name: percentile
                      args:
                        - Literal:
                            Integer: 50
                      named_args: {}
            return_ty: ~
        "###);
    }

    #[test]
    fn test_parse_sql_parameters() -> Result<()> {
        assert_yaml_snapshot!(parse(r#"
        from mytable
        filter [
          first_name == $1,
          last_name == $2.name
        ]
        "#)?, @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: mytable
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - List:
                          - Binary:
                              left:
                                Ident:
                                  namespace: ~
                                  name: first_name
                              op: Eq
                              right:
                                Ident:
                                  namespace: ~
                                  name: $1
                          - Binary:
                              left:
                                Ident:
                                  namespace: ~
                                  name: last_name
                              op: Eq
                              right:
                                Ident:
                                  namespace: $2
                                  name: name
                    named_args: {}
        "###);
        Ok(())
    }

    #[test]
    fn test_tab_characters() -> Result<()> {
        // #284

        let prql = "from c_invoice
join doc:c_doctype [~c_invoice_id]
select [
\tinvoice_no,
\tdocstatus
]";
        parse(prql)?;

        Ok(())
    }

    #[test]
    fn test_parse_backticks() -> Result<()> {
        let prql = "
from `a.b`
aggregate [max c]
join `my-proj.dataset.table`
join `my-proj`.`dataset`.`table`
";

        assert_yaml_snapshot!(parse(prql)?, @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: a.b
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  namespace: ~
                                  name: max
                              args:
                                - Ident:
                                    namespace: ~
                                    name: c
                              named_args: {}
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: join
                    args:
                      - Ident:
                          namespace: ~
                          name: my-proj.dataset.table
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: join
                    args:
                      - Ident:
                          namespace: my-proj.dataset
                          name: table
                    named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_parse_sort() -> Result<()> {
        assert_yaml_snapshot!(parse("
        from invoices
        sort issued_at
        sort (-issued_at)
        sort [issued_at]
        sort [-issued_at]
        sort [issued_at, -amount, +num_of_articles]
        ").unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: invoices
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: sort
                    args:
                      - Ident:
                          namespace: ~
                          name: issued_at
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: sort
                    args:
                      - Unary:
                          op: Neg
                          expr:
                            Ident:
                              namespace: ~
                              name: issued_at
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: sort
                    args:
                      - List:
                          - Ident:
                              namespace: ~
                              name: issued_at
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: sort
                    args:
                      - List:
                          - Unary:
                              op: Neg
                              expr:
                                Ident:
                                  namespace: ~
                                  name: issued_at
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: sort
                    args:
                      - List:
                          - Ident:
                              namespace: ~
                              name: issued_at
                          - Unary:
                              op: Neg
                              expr:
                                Ident:
                                  namespace: ~
                                  name: amount
                          - Ident:
                              namespace: ~
                              name: num_of_articles
                    named_args: {}
        "###);

        Ok(())
    }

    #[test]
    fn test_range() {
        assert_yaml_snapshot!(parse("
        from employees
        filter (age | between 18..40)
        derive [
          greater_than_ten = 11..,
          less_than_ten = ..9,
          negative = (-5..),
          more_negative = -10..,
          dates_open = @2020-01-01..,
          dates = @2020-01-01..@2021-01-01,
        ]
        ").unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: employees
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Pipeline:
                          exprs:
                            - Ident:
                                namespace: ~
                                name: age
                            - FuncCall:
                                name:
                                  Ident:
                                    namespace: ~
                                    name: between
                                args:
                                  - Range:
                                      start:
                                        Literal:
                                          Integer: 18
                                      end:
                                        Literal:
                                          Integer: 40
                                named_args: {}
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: derive
                    args:
                      - List:
                          - Range:
                              start:
                                Literal:
                                  Integer: 11
                              end: ~
                            alias: greater_than_ten
                          - Range:
                              start: ~
                              end:
                                Literal:
                                  Integer: 9
                            alias: less_than_ten
                          - Range:
                              start:
                                Literal:
                                  Integer: -5
                              end: ~
                            alias: negative
                          - Range:
                              start:
                                Literal:
                                  Integer: -10
                              end: ~
                            alias: more_negative
                          - Range:
                              start:
                                Literal:
                                  Date: 2020-01-01
                              end: ~
                            alias: dates_open
                          - Range:
                              start:
                                Literal:
                                  Date: 2020-01-01
                              end:
                                Literal:
                                  Date: 2021-01-01
                            alias: dates
                    named_args: {}
        "###);
    }

    #[test]
    fn test_dates() -> Result<()> {
        assert_yaml_snapshot!(parse("
        from employees
        derive [age_plus_two_years = (age + 2years)]
        ").unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: employees
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: derive
                    args:
                      - List:
                          - Binary:
                              left:
                                Ident:
                                  namespace: ~
                                  name: age
                              op: Add
                              right:
                                Literal:
                                  ValueAndUnit:
                                    n: 2
                                    unit: years
                            alias: age_plus_two_years
                    named_args: {}
        "###);

        assert_yaml_snapshot!(parse("
        derive [
            date = @2011-02-01,
            timestamp = @2011-02-01T10:00,
            time = @14:00,
            # datetime = @2011-02-01T10:00<datetime>,
        ]
        ").unwrap(), @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: derive
              args:
                - List:
                    - Literal:
                        Date: 2011-02-01
                      alias: date
                    - Literal:
                        Timestamp: "2011-02-01T10:00"
                      alias: timestamp
                    - Literal:
                        Time: "14:00"
                      alias: time
              named_args: {}
        "###);

        assert!(parse("derive x = @2020-01-0").is_err());

        Ok(())
    }

    #[test]
    fn test_multiline_string() {
        assert_yaml_snapshot!(parse(r###"
        derive x = r#"r-string test"#
        "###).unwrap(), @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: derive
              args:
                - Ident:
                    namespace: ~
                    name: r
                  alias: x
              named_args: {}
        "### )
    }

    #[test]
    fn test_parse_coalesce() {
        assert_yaml_snapshot!(parse(r###"
        from employees
        derive amount = amount ?? 0
        "###).unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: employees
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: derive
                    args:
                      - FuncCall:
                          name:
                            Ident:
                              namespace: ~
                              name: coalesce
                          args:
                            - Ident:
                                namespace: ~
                                name: amount
                            - Literal:
                                Integer: 0
                          named_args: {}
                        alias: amount
                    named_args: {}
        "### )
    }

    #[test]
    fn test_parse_literal() {
        assert_yaml_snapshot!(parse(r###"
        derive x = true
        "###).unwrap(), @r###"
        ---
        - Pipeline:
            FuncCall:
              name:
                Ident:
                  namespace: ~
                  name: derive
              args:
                - Literal:
                    Boolean: true
                  alias: x
              named_args: {}
        "###)
    }

    #[test]
    fn test_parse_allowed_idents() {
        assert_yaml_snapshot!(parse(r###"
        from employees
        join _salary [~employee_id] # table with leading underscore
        filter first_name == $1
        select [_employees._underscored_column]
        "###).unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: employees
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: join
                    args:
                      - Ident:
                          namespace: ~
                          name: _salary
                      - List:
                          - Unary:
                              op: EqSelf
                              expr:
                                Ident:
                                  namespace: ~
                                  name: employee_id
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              namespace: ~
                              name: first_name
                          op: Eq
                          right:
                            Ident:
                              namespace: ~
                              name: $1
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: select
                    args:
                      - List:
                          - Ident:
                              namespace: _employees
                              name: _underscored_column
                    named_args: {}
        "###)
    }

    #[test]
    fn test_parse_gt_lt_gte_lte() {
        assert_yaml_snapshot!(parse(r###"
        from people
        filter age >= 100
        filter num_grandchildren <= 10
        filter salary > 0
        filter num_eyes < 2
        "###).unwrap(), @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: from
                    args:
                      - Ident:
                          namespace: ~
                          name: people
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              namespace: ~
                              name: age
                          op: Gte
                          right:
                            Literal:
                              Integer: 100
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              namespace: ~
                              name: num_grandchildren
                          op: Lte
                          right:
                            Literal:
                              Integer: 10
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              namespace: ~
                              name: salary
                          op: Gt
                          right:
                            Literal:
                              Integer: 0
                    named_args: {}
                - FuncCall:
                    name:
                      Ident:
                        namespace: ~
                        name: filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              namespace: ~
                              name: num_eyes
                          op: Lt
                          right:
                            Literal:
                              Integer: 2
                    named_args: {}
        "###)
    }
}
