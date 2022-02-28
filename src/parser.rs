use std::vec;

use super::ast::*;
use super::utils::*;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
#[cfg(test)]
use pest::iterators::Pairs;
#[cfg(test)]
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

#[cfg(test)]
/// Parses a parse tree of pest Pairs into an AST.
fn ast_of_parse_tree(pairs: Pairs<Rule>) -> Result<Items> {
    pairs
        // Exclude end-of-input at the moment.
        .filter(|pair| pair.as_rule() != Rule::EOI)
        .map(|pair| {
            // TODO: Probably wrap each of the individual branches in a Result,
            // and don't have this wrapping `Ok`. Then move some of the panics
            // to `Err`s.
            Ok(match pair.as_rule() {
                Rule::query => Item::Query(ast_of_parse_tree(pair.into_inner())?),
                Rule::list => Item::List(ast_of_parse_tree(pair.into_inner())?),
                Rule::idents => {
                    Item::Idents(pair.into_inner().map(|x| x.as_str().to_owned()).collect())
                }
                Rule::items => Item::Items(ast_of_parse_tree(pair.into_inner())?)
                    // We collapse any Items with a single element into that
                    // element. We don't do this with Expr or List because those are
                    // often meaningful — e.g. a List needs a number of Expr, so that
                    // `[a, b]` is different from `[a b]`.
                    .into_item(),
                Rule::expr => Item::Expr(ast_of_parse_tree(pair.into_inner())?),
                Rule::named_arg => {
                    let parsed: [Item; 2] = ast_of_parse_tree(pair.into_inner())?
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {:?}", e))?;
                    let [name, arg] = parsed;
                    Item::NamedArg(NamedArg {
                        name: name.as_ident()?.to_owned(),
                        arg: Box::new(arg),
                    })
                }
                Rule::assign => {
                    let parsed: [Item; 2] = ast_of_parse_tree(pair.into_inner())?
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {:?}", e))?;
                    // Split the pair into its first value, which is always an Ident,
                    // and its other values.
                    if let [lvalue, Item::Expr(rvalue)] = parsed {
                        Ok(Item::Assign(Assign {
                            lvalue: lvalue.as_ident()?.to_owned(),
                            rvalue: Box::new(Item::Items(rvalue).into_item()),
                        }))
                    } else {
                        Err(anyhow!(
                            "Expected assign to have an lvalue & some rvalues. Got {:?}",
                            parsed
                        ))
                    }?
                }
                Rule::transformation => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;
                    Item::Transformation(parsed.try_into()?)
                }
                Rule::function => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;
                    if let (Item::Idents(name_and_params), body) = parsed
                        .split_first()
                        .ok_or(anyhow!("Expceted at least one item"))?
                    {
                        let (name, args) = name_and_params
                            .split_first()
                            .ok_or(anyhow!("Expected at least one item"))?;
                        Item::Function(Function {
                            name: name.to_owned(),
                            args: args.to_owned(),
                            body: body.to_owned(),
                        })
                    } else {
                        unreachable!("Expected Function, got {:?}", 1)
                    }
                }
                Rule::table => {
                    let parsed = ast_of_parse_tree(pair.into_inner())?;
                    if let [name, Item::Pipeline(pipeline)] = &parsed[..] {
                        Item::Table(Table {
                            name: name.as_ident()?.to_owned(),
                            pipeline: pipeline.clone(),
                        })
                    } else {
                        unreachable!("Expected Table, got {:?}", parsed)
                    }
                }
                Rule::ident => Item::Ident(pair.as_str().to_string()),
                // Pull out the string itself, which doesn't have the quotes
                Rule::string_literal => ast_of_parse_tree(pair.clone().into_inner())?
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("Failed reading string {:?}", &pair))?,
                Rule::string => Item::String(pair.as_str().to_string()),
                Rule::s_string => Item::SString(
                    pair.into_inner()
                        // TODO: change unwraps to results (requires some more
                        // verbose code given it's inside an expression inside a `map`)
                        .map(|x| match x.as_rule() {
                            Rule::s_string_string => SStringItem::String(x.as_str().to_string()),
                            _ => SStringItem::Expr(Item::Items(
                                ast_of_parse_tree(x.into_inner()).unwrap(),
                            )),
                        })
                        .collect(),
                ),
                Rule::pipeline => Item::Pipeline({
                    ast_of_parse_tree(pair.into_inner())?
                        .into_iter()
                        .map(|x| match x {
                            Item::Transformation(transformation) => transformation,
                            _ => unreachable!("{:?}", x),
                        })
                        .collect()
                }),
                Rule::operator | Rule::number => Item::Raw(pair.as_str().to_owned()),
                _ => (Item::Todo(pair.as_str().to_owned())),
            })
        })
        .collect()
}

/// Parse a string into a parse tree, made up of pest Pairs.
#[cfg(test)]
fn parse_tree_of_str(source: &str, rule: Rule) -> Result<Pairs<Rule>> {
    PrqlParser::parse(rule, source).map_err(|e| e.into())
}

// We put this outside the main ast_of_parse_tree function because we also use it to ast_of_parse_tree
// function calls.
// (I'm not sure whether we should be using it for both — on the one hand,
// they're fairly similar `sum salary` is a standard function call. But on the
// other, we were planning to allow `salary | sum`, which doesn't work. We would
// need to parse the whole of `sum salary` as a pipeline, which _might_ then work.)
impl TryFrom<Vec<Item>> for Transformation {
    type Error = anyhow::Error;
    fn try_from(items: Vec<Item>) -> Result<Self> {
        // if let Item::Items(items) = items {
        let (name_item, all_args) = items
            .split_first()
            .ok_or(anyhow!("Expected at least one item"))?;
        let name = name_item.as_ident()?;

        let (named_arg_items, args): (Vec<Item>, Vec<Item>) = all_args
            .iter()
            // This reduces the nesting.
            .map(|x| x.as_item())
            .cloned()
            .partition(|x| matches!(x, Item::NamedArg(_)));

        let named_args: Vec<NamedArg> = named_arg_items
            .iter()
            .map(|x| x.as_named_arg().cloned())
            .try_collect()?;

        match name.as_str() {
            "from" => Ok(Transformation::From(args)),
            "select" => Ok(Transformation::Select(args)),
            "filter" => Ok(Transformation::Filter(Filter(args))),
            "derive" => {
                let assigns = args
                    .into_only()
                    .context("Expected at least one argument")?
                    .into_unnested()
                    .as_inner_items()?
                    .iter()
                    // TODO: couldn't manage to avoid cloning here.
                    .map(|x| x.as_assign().cloned())
                    .try_collect()?;
                Ok(Transformation::Derive(assigns))
            }
            "aggregate" => {
                // This is more compicated rust than I was expecting, and we may
                // generalize these checks to Func functions anyway.
                if args.len() != 1 {
                    return Err(anyhow!(
                        "Expected exactly one unnamed argument for aggregate"
                    ));
                }
                let by = match &named_args[..] {
                    [NamedArg { name, arg }] => {
                        if name != "by" {
                            return Err(anyhow!(
                                "Expected aggregate to have up to one named arg, named 'by'"
                            ));
                        }
                        (*arg).clone().into_items()
                    }
                    [] => vec![],
                    _ => {
                        return Err(anyhow!(
                            "Expected aggregate to have up to one named arg, named 'by'"
                        ))
                    }
                };

                let ops = args
                    .first()
                    .ok_or_else(|| anyhow!("Failed on {:?}", args))
                    .cloned()
                    .map(|x| x.into_items())?;

                // Ops should either be calcs or assigns; e.g. one of
                //   average gross_cost
                //   sum_gross_cost: sum gross_cost

                let (assigns, calcs): (Vec<Item>, Vec<Item>) = ops
                    .iter()
                    .cloned()
                    .partition(|x| matches!(x, Item::Assign(_)));

                Ok(Transformation::Aggregate {
                    by,
                    calcs,
                    assigns: assigns
                        .into_iter()
                        .map(|x| {
                            // The assigns need to be parsed as Transformations.
                            // Potentially there's a nicer way of doing this in
                            // Rust, so we're not parsing them one way and then
                            // parsing them another. (I thought about having
                            // Assign generic in its rvalue, but then Item needs
                            // that generic parameter too?)
                            x.as_assign().cloned().map(|assign| Assign {
                                lvalue: assign.lvalue,
                                // Make the rvalue items into a transformation.
                                rvalue: Box::new(Item::Transformation(
                                    assign.rvalue.into_items().try_into().unwrap(),
                                )),
                            })
                        })
                        .try_collect()?,
                })
            }
            "sort" => Ok(Transformation::Sort(args)),
            "take" => {
                // TODO: coerce to number
                args.into_only()
                    .map(|x| x.as_item().clone())
                    .map(|n| Ok(Transformation::Take(n.as_raw()?.parse()?)))?
            }
            "join" => Ok(Transformation::Join(args)),
            _ => Ok(Transformation::Func(FuncCall {
                name: name.to_owned(),
                args,
                named_args,
            })),
        }
    }
}

/// Parse a string into an AST.
#[cfg(test)]
pub fn ast_of_string(string: &str, rule: Rule) -> Result<Item> {
    parse_tree_of_str(string, rule)
        .and_then(ast_of_parse_tree)
        .and_then(|x| x.into_only())
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    #[test]
    fn test_parse_take() -> Result<()> {
        assert!(parse_tree_of_str("take 10", Rule::transformation).is_ok());
        assert!(ast_of_string("take", Rule::transformation).is_err());
        Ok(())
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
        assert_yaml_snapshot!(ast_of_string(r#"" U S A ""#, Rule::string_literal)?, @r###"
        ---
        String: " U S A "
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_s_string() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"s"SUM({col})""#, Rule::s_string)?);
        assert_yaml_snapshot!(ast_of_string(r#"s"SUM({col})""#, Rule::s_string)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Items:
                - Ident: col
          - String: )
        "###);
        assert_yaml_snapshot!(ast_of_string(r#"s"SUM({2 + 2})""#, Rule::s_string)?, @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Items:
                - Raw: "2"
                - Raw: +
                - Raw: "2"
          - String: )
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_list() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"[1 + 1, 2]"#, Rule::list)?);
        assert_yaml_snapshot!(ast_of_string(r#"[1 + 1, 2]"#, Rule::list)?, @r###"
        ---
        List:
          - Expr:
              - Raw: "1"
              - Raw: +
              - Raw: "1"
          - Expr:
              - Raw: "2"
        "###);
        let ab = ast_of_string(r#"[a b]"#, Rule::list)?;
        let a_comma_b = ast_of_string(r#"[a, b]"#, Rule::list)?;
        assert_yaml_snapshot!(ab, @r###"
        ---
        List:
          - Expr:
              - Items:
                  - Ident: a
                  - Ident: b
        "###);
        assert_yaml_snapshot!(a_comma_b, @r###"
        ---
        List:
          - Expr:
              - Ident: a
          - Expr:
              - Ident: b
        "###);
        assert_ne!(ab, a_comma_b);
        Ok(())
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
    fn test_parse_filter() -> Result<()> {
        assert_yaml_snapshot!(
            ast_of_string(r#"filter country = "USA""#, Rule::transformation)?
        , @r###"
        ---
        Transformation:
          Filter:
            - Expr:
                - Ident: country
                - Raw: "="
                - String: USA
        "###);
        assert_yaml_snapshot!(
            ast_of_string(r#"filter (upper country) = "USA""#, Rule::transformation)?
        , @r###"
        ---
        Transformation:
          Filter:
            - Expr:
                - Items:
                    - Ident: upper
                    - Ident: country
                - Raw: "="
                - String: USA
        "###);
        assert_yaml_snapshot!(
            ast_of_string(r#"filter upper country = "USA""#, Rule::transformation)?
        , @r###"
        ---
        Transformation:
          Filter:
            - Expr:
                - Items:
                    - Ident: upper
                    - Ident: country
                - Raw: "="
                - String: USA
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
        assert_yaml_snapshot!(
            ast_of_string("aggregate by:[title] [sum salary]", Rule::transformation)?, @r###"
        ---
        Transformation:
          Aggregate:
            by: []
            calcs:
              - NamedArg:
                  name: by
                  arg:
                    List:
                      - Expr:
                          - Ident: title
              - List:
                  - Expr:
                      - Items:
                          - Ident: sum
                          - Ident: salary
            assigns: []
        "###);
        assert_yaml_snapshot!(ast_of_string(
                r#"[
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
]"#,
        Rule::list)?, @r###"
        ---
        List:
          - Expr:
              - Assign:
                  lvalue: gross_salary
                  rvalue:
                    Items:
                      - Ident: salary
                      - Raw: +
                      - Ident: payroll_tax
          - Expr:
              - Assign:
                  lvalue: gross_cost
                  rvalue:
                    Items:
                      - Ident: gross_salary
                      - Raw: +
                      - Ident: benefits_cost
        "###);
        assert_yaml_snapshot!(
            ast_of_string(
                "gross_salary: (salary + payroll_tax) * (1 + tax_rate)",
                Rule::expr,
            )?,
            @r###"
        ---
        Expr:
          - Assign:
              lvalue: gross_salary
              rvalue:
                Items:
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
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
]
sort sum_gross_cost
filter count > 200
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
            Rule::function
        )?);
        assert_yaml_snapshot!(ast_of_string(
            "func identity x = x", Rule::function
        )?
        , @r###"
        ---
        Function:
          name: identity
          args:
            - x
          body:
            - Ident: x
        "###);
        assert_yaml_snapshot!(ast_of_string(
            "func plus_one x = (x + 1)", Rule::function
        )?
        , @r###"
        ---
        Function:
          name: plus_one
          args:
            - x
          body:
            - Expr:
                - Ident: x
                - Raw: +
                - Raw: "1"
        "###);
        assert_yaml_snapshot!(ast_of_string(
            "func plus_one x = x + 1", Rule::function
        )?
        , @r###"
        ---
        Function:
          name: plus_one
          args:
            - x
          body:
            - Ident: x
            - Raw: +
            - Raw: "1"
        "###);
        // An example to show that we can't delayer the tree, despite there
        // being lots of layers.
        assert_yaml_snapshot!(ast_of_string(
            "func foo x = (foo bar + 1) (plax) - baz", Rule::function
        )?
        , @r###"
        ---
        Function:
          name: foo
          args:
            - x
          body:
            - Items:
                - Expr:
                    - Items:
                        - Ident: foo
                        - Ident: bar
                    - Raw: +
                    - Raw: "1"
                - Expr:
                    - Ident: plax
            - Raw: "-"
            - Ident: baz
        "###);

        assert_yaml_snapshot!(ast_of_string("func return_constant = 42", Rule::function)?, @r###"
        ---
        Function:
          name: return_constant
          args: []
          body:
            - Raw: "42"
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
                    Rule::function
                )
                .unwrap()
            ));
            */
        Ok(())
    }

    #[test]
    fn test_parse_table() -> Result<()> {
        assert_yaml_snapshot!(ast_of_string(
            "table newest_employees = ( from employees )",
            Rule::table
        )?, @r###"
        ---
        Table:
          name: newest_employees
          pipeline:
            - From:
                - Ident: employees
        "###);

        assert_yaml_snapshot!(ast_of_string(
            r#"
        table newest_employees = (
          from employees
          sort tenure
          take 50
        )"#.trim(), Rule::table)?,
         @r###"
        ---
        Table:
          name: newest_employees
          pipeline:
            - From:
                - Ident: employees
            - Sort:
                - Ident: tenure
            - Take: 50
        "###);
        Ok(())
    }

    #[test]
    fn test_parse_into_parse_tree() -> Result<()> {
        assert_debug_snapshot!(parse_tree_of_str(r#"country = "USA""#, Rule::expr)?);
        assert_debug_snapshot!(parse_tree_of_str("select [a, b, c]", Rule::transformation)?);
        assert_debug_snapshot!(parse_tree_of_str(
            "aggregate by:[title, country] [sum salary]",
            Rule::transformation
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"    filter country = "USA""#,
            Rule::transformation
        )?);
        assert_debug_snapshot!(parse_tree_of_str("[a, b, c,]", Rule::list)?);
        assert_debug_snapshot!(parse_tree_of_str(
            r#"[
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
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
            Rule::transformation
        )?);
        assert_debug_snapshot!(parse_tree_of_str(
            "join side:left country [id=employee_id]",
            Rule::transformation
        )?);
        assert_debug_snapshot!(parse_tree_of_str("1  + 2", Rule::expr)?);
        Ok(())
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
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
]
sort sum_gross_cost
filter count > 200
take 20
    "#
            .trim(),
            Rule::pipeline
        )?);
        Ok(())
    }
}
