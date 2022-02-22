use std::vec;

use super::ast::*;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use pest::error::Error;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

pub fn parse(pairs: Pairs<Rule>) -> Result<Items> {
    pairs
        .map(|pair| {
            // TODO: Probably wrap each of the individual branches in a Result,
            // and don't have this wrapping `Ok`. Then move some of the panics
            // to `Err`s.
            Ok(match pair.as_rule() {
                Rule::list => Item::List(parse(pair.into_inner())?),
                Rule::items => Item::Items(parse(pair.into_inner())?),
                Rule::named_arg => {
                    let parsed = parse(pair.into_inner())?;
                    // Split the pair into its first value, which is always an Ident,
                    // and its second value, which can be an ident / list / etc.
                    if let [name, arg] = &parsed[..] {
                        Ok(Item::NamedArg(NamedArg {
                            name: name.as_ident()?.to_owned(),
                            arg: Box::new(arg.clone()),
                        }))
                    } else {
                        Err(anyhow!(
                            "Expected NamedArg to have an name & arg. Got {:?}",
                            parsed
                        ))
                    }?
                }
                Rule::assign => {
                    let parsed = parse(pair.into_inner())?;
                    // Split the pair into its first value, which is always an Ident,
                    // and its other values.
                    if let [lvalue, Item::Items(rvalue)] = &parsed[..] {
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
                    let parsed = parse(pair.into_inner())?;
                    if let [Item::Idents(name_and_params), Item::Items(body)] = &parsed[..] {
                        let (name, params) = name_and_params.split_first().unwrap();
                        Item::Function(Function {
                            name: name.to_owned(),
                            args: params.to_owned(),
                            body: body.to_owned(),
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
                Rule::string => Item::String(pair.as_str().to_string()),
                Rule::query => Item::Query(parse(pair.into_inner())?),
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

pub fn parse_to_pest_tree(source: &str, rule: Rule) -> Result<Pairs<Rule>, Error<Rule>> {
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

                let x = Transformation::Aggregate {
                    by,
                    calcs: calcs
                        .iter()
                        .cloned()
                        .map(|x| TryInto::<Transformation>::try_into(x.to_items()))
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
                                    TryInto::<Transformation>::try_into(assign.rvalue.to_items())
                                        .unwrap(),
                                )),
                            })
                        })
                        .try_collect()?,
                };
                Ok(x)
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
            _ => Ok(Transformation::Func {
                name: name.to_owned(),
                args,
                named_args,
            }),
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
                - Func:
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
                    rule: string,
                    span: Span {
                        str: "USA",
                        start: 11,
                        end: 14,
                    },
                    inner: [],
                },
            ],
        )
        "###);
        assert_debug_snapshot!(parse_to_pest_tree(r#"USA"#, Rule::string));
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
        assert_debug_snapshot!(parse_to_pest_tree("1 + 2", Rule::expr), @r###"
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
