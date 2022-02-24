use std::vec;

use super::ast::*;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

pub fn parse(pairs: Pairs<Rule>) -> Result<Items> {
    pairs
        // Exclude end-of-input at the moment.
        .filter(|pair| pair.as_rule() != Rule::EOI)
        .map(|pair| {
            // TODO: Probably wrap each of the individual branches in a Result,
            // and don't have this wrapping `Ok`. Then move some of the panics
            // to `Err`s.
            Ok(match pair.as_rule() {
                Rule::query => Item::Query(parse(pair.into_inner())?),
                Rule::list => Item::List(parse(pair.into_inner())?),
                Rule::items => Item::Items(parse(pair.into_inner())?),
                Rule::named_arg => {
                    let parsed: [Item; 2] = parse(pair.into_inner())?
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {:?}", e))?;
                    let [name, arg] = parsed;
                    Item::NamedArg(NamedArg {
                        name: name.as_ident()?.to_owned(),
                        arg: Box::new(arg),
                    })
                }
                Rule::assign => {
                    let parsed: [Item; 2] = parse(pair.into_inner())?
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {:?}", e))?;
                    // Split the pair into its first value, which is always an Ident,
                    // and its other values.
                    if let [lvalue, Item::Items(rvalue)] = parsed {
                        Ok(Item::Assign(Assign {
                            lvalue: lvalue.as_ident()?.to_owned(),
                            rvalue: Box::new(Item::Items(rvalue.to_vec())),
                        }))
                    } else {
                        Err(anyhow!(
                            "Expected assign to have an lvalue & some rvalues. Got {:?}",
                            parsed
                        ))
                    }?
                }
                Rule::transformation => {
                    let parsed = parse(pair.into_inner())?;
                    Item::Transformation(parsed.try_into()?)
                }
                Rule::function => {
                    let parsed: [Item; 2] = parse(pair.into_inner())?
                        .try_into()
                        .map_err(|e| anyhow!("Expected two items; {:?}", e))?;
                    if let [Item::Idents(name_and_params), Item::Items(body)] = parsed {
                        let (name, params) = name_and_params.split_first().unwrap();
                        Item::Function(Function {
                            name: name.to_owned(),
                            args: params.to_owned(),
                            body,
                        })
                    } else {
                        unreachable!("Expected Function, got {:?}", parsed)
                    }
                }
                Rule::table => {
                    let parsed = parse(pair.into_inner())?;
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
                Rule::idents => Item::Idents(
                    parse(pair.into_inner())?
                        .into_iter()
                        .map(|x| x.as_ident().cloned())
                        .try_collect()?,
                ),
                // Pull out the string itself, which doesn't have the quotes
                Rule::string_literal => parse(pair.clone().into_inner())?
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("Failed reading string {:?}", &pair))?,
                Rule::string => Item::String(pair.as_str().to_string()),
                Rule::s_string => Item::SString(
                    pair.into_inner()
                        .map(|x| match x.as_rule() {
                            Rule::s_string_string => SStringItem::String(x.as_str().to_string()),
                            _ => SStringItem::Expr(Item::Items(parse(x.into_inner()).unwrap())),
                        })
                        .collect(),
                ),
                Rule::pipeline => Item::Pipeline({
                    parse(pair.into_inner())?
                        .into_iter()
                        .map(|x| match x {
                            Item::Transformation(transformation) => transformation,
                            _ => unreachable!("{:?}", x),
                        })
                        .collect()
                }),
                Rule::operator | Rule::number => Item::Raw(pair.as_str().to_owned()),
                _ => (Item::TODO(pair.as_str().to_owned())),
            })
        })
        .collect()
}

pub fn parse_to_pest_tree(source: &str, rule: Rule) -> Result<Pairs<Rule>> {
    let pairs = PrqlParser::parse(rule, source)?;
    Ok(pairs)
}

// We put this outside the main parse function because we also use it to parse
// function calls.
// (I'm not sure whether we should be using it for both — on the one hand,
// they're fairly similar `sum salary` is a standard function call. But on the
// other, we were planning to allow `salary | sum`, which doesn't work. We would
// need to parse the whole of `sum salary` as a pipeline, which _might_ then work.)
impl TryFrom<Vec<Item>> for Transformation {
    type Error = anyhow::Error;
    fn try_from(items: Vec<Item>) -> Result<Self> {
        let (name_item, all_args) = items.split_first().unwrap();
        let name = name_item.as_ident()?;

        let (named_arg_items, args): (Vec<Item>, Vec<Item>) = all_args
            .iter()
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
                    .first()
                    .context("Expected at least one argument")?
                    .to_items()
                    .iter()
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
                        (*arg).to_items()
                    }
                    [] => vec![],
                    _ => {
                        return Err(anyhow!(
                            "Expected aggregate to have up to one named arg, named 'by'"
                        ))
                    }
                };

                let ops = args.first().unwrap().to_items();

                // Ops should either be calcs or assigns; e.g. one of
                //   average gross_cost
                //   sum_gross_cost: sum gross_cost

                let (assigns, calcs): (Vec<Item>, Vec<Item>) = ops
                    .iter()
                    .cloned()
                    .partition(|x| matches!(x, Item::Assign(_)));

                Ok(Transformation::Aggregate {
                    by,
                    calcs: calcs
                        .iter()
                        .cloned()
                        .map(|x| x.to_items().try_into().map(Item::Transformation))
                        .try_collect()?,
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
                                    assign.rvalue.to_items().try_into().unwrap(),
                                )),
                            })
                        })
                        .try_collect()?,
                })
            }
            "sort" => Ok(Transformation::Sort(args)),
            "take" => {
                // TODO confirm there is only one arg
                match args.first() {
                    // TODO: coerce to number
                    Some(n) => Ok(Transformation::Take(n.as_raw()?.parse()?)),
                    None => Err(anyhow!("Expected a number; got {:?}", args)),
                }
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

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    #[test]
    fn test_parse_take() {
        assert!(parse_to_pest_tree("take 10", Rule::transformation).is_ok());
        assert!(parse_to_pest_tree("take", Rule::transformation).is_err());
    }

    #[test]
    fn test_parse_string() {
        assert_debug_snapshot!(parse_to_pest_tree(r#"" U S A ""#, Rule::string_literal).unwrap(), @r###"
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
        assert_yaml_snapshot!(parse(parse_to_pest_tree(r#"" U S A ""#, Rule::string_literal).unwrap()).unwrap(), @r###"
        ---
        - String: " U S A "
        "###);
    }

    #[test]
    fn test_parse_s_string() {
        assert_debug_snapshot!(parse_to_pest_tree(r#"s"SUM({col})""#, Rule::s_string), @r###"
        Ok(
            [
                Pair {
                    rule: s_string,
                    span: Span {
                        str: "s\"SUM({col})\"",
                        start: 0,
                        end: 13,
                    },
                    inner: [
                        Pair {
                            rule: s_string_string,
                            span: Span {
                                str: "SUM(",
                                start: 2,
                                end: 6,
                            },
                            inner: [],
                        },
                        Pair {
                            rule: items,
                            span: Span {
                                str: "col",
                                start: 7,
                                end: 10,
                            },
                            inner: [
                                Pair {
                                    rule: ident,
                                    span: Span {
                                        str: "col",
                                        start: 7,
                                        end: 10,
                                    },
                                    inner: [],
                                },
                            ],
                        },
                        Pair {
                            rule: s_string_string,
                            span: Span {
                                str: ")",
                                start: 11,
                                end: 12,
                            },
                            inner: [],
                        },
                    ],
                },
            ],
        )
        "###);
        assert_yaml_snapshot!(parse(parse_to_pest_tree(r#"s"SUM({col})""#, Rule::s_string).unwrap()).unwrap(), @r###"
        ---
        - SString:
            - String: SUM(
            - Expr:
                Items:
                  - Ident: col
            - String: )
        "###);
        assert_yaml_snapshot!(parse(parse_to_pest_tree(r#"s"SUM({2 + 2})""#, Rule::s_string).unwrap()).unwrap(), @r###"
        ---
        - SString:
            - String: SUM(
            - Expr:
                Items:
                  - Raw: "2"
                  - Raw: +
                  - Raw: "2"
            - String: )
        "###);
    }

    #[test]
    fn test_parse_number() {
        assert_debug_snapshot!(parse_to_pest_tree(r#"23"#, Rule::number), @r###"
        Ok(
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
            ],
        )
        "###);
        assert_debug_snapshot!(parse_to_pest_tree(r#"2 + 2"#, Rule::expr), @r###"
        Ok(
            [
                Pair {
                    rule: number,
                    span: Span {
                        str: "2",
                        start: 0,
                        end: 1,
                    },
                    inner: [],
                },
                Pair {
                    rule: operator,
                    span: Span {
                        str: "+",
                        start: 2,
                        end: 3,
                    },
                    inner: [],
                },
                Pair {
                    rule: number,
                    span: Span {
                        str: "2",
                        start: 4,
                        end: 5,
                    },
                    inner: [],
                },
            ],
        )
        "###);
    }

    #[test]
    fn test_parse_expr() {
        assert_yaml_snapshot!(
            parse(parse_to_pest_tree(r#"country = "USA""#, Rule::expr).unwrap()).unwrap()
        , @r###"
        ---
        - Ident: country
        - Raw: "="
        - String: USA
        "###);
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("aggregate by:[title] [sum salary]", Rule::transformation).unwrap()
        )
        .unwrap(), @r###"
        ---
        - Transformation:
            Aggregate:
              by:
                - Ident: title
              calcs:
                - Transformation:
                    Func:
                      name: sum
                      args:
                        - Ident: salary
                      named_args: []
              assigns: []
        "###);
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree(
                r#"[
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
]"#,
                Rule::list,
            )
            .unwrap()
        )
        .unwrap(), @r###"
        ---
        - List:
            - Assign:
                lvalue: gross_salary
                rvalue:
                  Items:
                    - Ident: salary
                    - Raw: +
                    - Ident: payroll_tax
            - Assign:
                lvalue: gross_cost
                rvalue:
                  Items:
                    - Ident: gross_salary
                    - Raw: +
                    - Ident: benefits_cost
        "###);
        assert_yaml_snapshot!(parse(
                    parse_to_pest_tree(
                        "gross_salary: (salary + payroll_tax) * (1 + tax_rate)",
                        Rule::expr,
                    )
                    .unwrap()
                )
                .unwrap(), @r###"
        ---
        - Assign:
            lvalue: gross_salary
            rvalue:
              Items:
                - Items:
                    - Ident: salary
                    - Raw: +
                    - Ident: payroll_tax
                - Raw: "*"
                - Items:
                    - Raw: "1"
                    - Raw: +
                    - Ident: tax_rate
        "###);
    }

    #[test]
    fn test_parse_pipeline() {
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree(
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
                Rule::pipeline,
            )
            .unwrap()
        )
        .unwrap());
    }

    #[test]
    fn test_parse_function() {
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("func identity x = x", Rule::function).unwrap()
        )
        .unwrap(), @r###"
        ---
        - Function:
            name: identity
            args:
              - x
            body:
              - Ident: x
        "###);

        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("func plus_one x = x + 1", Rule::function).unwrap()
        )
        .unwrap(), @r###"
        ---
        - Function:
            name: plus_one
            args:
              - x
            body:
              - Ident: x
              - Raw: +
              - Raw: "1"
        "###);

        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("func return_constant = 42", Rule::function).unwrap()
        )
        .unwrap(), @r###"
        ---
        - Function:
            name: return_constant
            args: []
            body:
              - Raw: "42"
        "###);

        /* TODO: Does not yet parse because `window` not yet implemented.
            assert_debug_snapshot!(parse(
                parse_to_pest_tree(
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
    }

    #[test]
    fn test_parse_table() {
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("table newest_employees = ( from employees )",
                Rule::table
            )
            .unwrap()
        )
        .unwrap(), @r###"
        ---
        - Table:
            name: newest_employees
            pipeline:
              - From:
                  - Ident: employees
        "###);
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree(r#"
        table newest_employees = (
          from employees
          sort tenure
          take 50
        )
                "#.trim(),
                Rule::table
            )
            .unwrap()
        )
        .unwrap(), @r###"
        ---
        - Table:
            name: newest_employees
            pipeline:
              - From:
                  - Ident: employees
              - Sort:
                  - Ident: tenure
              - Take: 50
        "###);
    }

    #[test]
    fn test_parse_to_pest_tree() {
        assert_debug_snapshot!(parse_to_pest_tree(r#"country = "USA""#, Rule::expr), @r###"
        Ok(
            [
                Pair {
                    rule: ident,
                    span: Span {
                        str: "country",
                        start: 0,
                        end: 7,
                    },
                    inner: [],
                },
                Pair {
                    rule: operator,
                    span: Span {
                        str: "=",
                        start: 8,
                        end: 9,
                    },
                    inner: [],
                },
                Pair {
                    rule: string_literal,
                    span: Span {
                        str: "\"USA\"",
                        start: 10,
                        end: 15,
                    },
                    inner: [
                        Pair {
                            rule: string,
                            span: Span {
                                str: "USA",
                                start: 11,
                                end: 14,
                            },
                            inner: [],
                        },
                    ],
                },
            ],
        )
        "###);
        assert_debug_snapshot!(parse_to_pest_tree(r#""USA""#, Rule::string_literal));
        assert_debug_snapshot!(parse_to_pest_tree("select [a, b, c]", Rule::transformation));
        assert_debug_snapshot!(parse_to_pest_tree(
            "aggregate by:[title, country] [sum salary]",
            Rule::transformation
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"    filter country = "USA""#,
            Rule::transformation
        ));
        assert_debug_snapshot!(parse_to_pest_tree("[a, b, c,]", Rule::list));
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"[
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
]"#,
            Rule::list
        ));
        // Currently not putting comments in our parse tree, so this is blank.
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"# this is a comment
        select a"#,
            Rule::COMMENT
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            "join country [id=employee_id]",
            Rule::transformation
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            "join side:left country [id=employee_id]",
            Rule::transformation
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            "join side:right country [id=employee_id]",
            Rule::transformation
        ));
        assert_debug_snapshot!(parse_to_pest_tree("1  + 2", Rule::expr), @r###"
        Ok(
            [
                Pair {
                    rule: number,
                    span: Span {
                        str: "1",
                        start: 0,
                        end: 1,
                    },
                    inner: [],
                },
                Pair {
                    rule: operator,
                    span: Span {
                        str: "+",
                        start: 3,
                        end: 4,
                    },
                    inner: [],
                },
                Pair {
                    rule: number,
                    span: Span {
                        str: "2",
                        start: 5,
                        end: 6,
                    },
                    inner: [],
                },
            ],
        )
        "###);
    }

    #[test]
    fn test_parse_to_pest_tree_pipeline() {
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"
    from employees
    select [a, b]
    "#
            .trim(),
            Rule::pipeline
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"
    from employees
    filter country = "USA"
    "#
            .trim(),
            Rule::pipeline
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
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
        ));
    }
}
