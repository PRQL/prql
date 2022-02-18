use super::ast::*;
use anyhow::{Context, Result};
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
            Ok(match pair.as_rule() {
                Rule::list => Item::List(parse(pair.into_inner())?),
                Rule::items => Item::Items(parse(pair.into_inner())?),
                Rule::named_arg => {
                    let parsed = parse(pair.into_inner())?;
                    // Split the pair into its first value, which is always an Ident,
                    // and the rest of the values.
                    let (lvalue, rvalue) = parsed.split_first().unwrap();

                    Item::NamedArg(NamedArg {
                        lvalue: lvalue.as_ident()?.to_owned(),
                        rvalue: rvalue.to_vec(),
                    })
                }
                Rule::assign => {
                    let parsed = parse(pair.into_inner())?;
                    if let [lvalue, Item::Items(rvalue)] = &parsed[..] {
                        Item::Assign(Assign {
                            lvalue: lvalue.as_ident()?.to_owned(),
                            rvalue: rvalue.to_vec(),
                        })
                    } else {
                        panic!("Expected Assign, got {:?}", parsed)
                    }
                }
                Rule::transformation => {
                    let parsed = parse(pair.into_inner())?;
                    let (name_item, all_args) = parsed.split_first().unwrap();
                    let name = name_item.as_ident()?;

                    let (named_arg_items, args): (Vec<Item>, Vec<Item>) = all_args
                        .iter()
                        .cloned()
                        .partition(|x| matches!(x, Item::NamedArg(_)));

                    let named_args: Vec<NamedArg> = named_arg_items
                        .iter()
                        .map(|x| x.as_named_arg().cloned())
                        .collect::<Result<Vec<_>>>()?;

                    let transformation = match name.as_str() {
                        "from" => Transformation::From(args),
                        "select" => Transformation::Select(args),
                        "filter" => Transformation::Filter(args),
                        "derive" => {
                            let assigns = args
                                .first()
                                .context("Expected at least one argument")?
                                .to_items()
                                .iter()
                                .map(|x| x.as_assign().cloned())
                                .collect::<Result<Vec<_>>>()?;
                            Transformation::Derive(assigns)
                        }
                        "aggregate" => Transformation::Aggregate {
                            calcs: args.to_vec(),
                            by: named_args
                                .iter()
                                .find(|x| x.lvalue == "by")
                                .map(|x| x.rvalue.clone())
                                .unwrap_or_else(Vec::new),
                        },
                        "sort" => Transformation::Sort(args),
                        "take" => Transformation::Take({
                            match args.first() {
                                // TODO: coerce to number
                                Some(n) => n.as_raw()?.parse()?,
                                // TOOD: Raise here
                                // None => return anyhow!("Expected a number"),
                                None => unimplemented!(),
                            }
                        }),
                        "join" => Transformation::Join(args),
                        _ => Transformation::Custom {
                            name: name.to_owned(),
                            args,
                            named_args,
                        },
                    };
                    Item::Transformation(transformation)
                    // Item::Transformation(Transformation {
                    //     name: name.as_ident()?.as_str().into(),
                    //     args,
                    //     named_args: named_args
                    //         .iter()
                    //         .map(|x| x.as_named_arg().cloned())
                    //         .collect::<Result<_>>()?,
                    // })
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
                        .collect::<Result<Vec<_>>>()?,
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

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    #[test]
    fn test_parse_expr() {
        assert_yaml_snapshot!(
            parse(parse_to_pest_tree(r#"country = "USA""#, Rule::expr).unwrap()).unwrap()
        , @r###"
        ---
        - Ident: country
        - Raw: "="
        - String: "\"USA\""
        "###);
        assert_yaml_snapshot!(parse(
            parse_to_pest_tree("aggregate by:[title] [sum salary]", Rule::transformation).unwrap()
        )
        .unwrap(), @r###"
        ---
        - Transformation:
            Aggregate:
              by:
                - List:
                    - Ident: title
              calcs:
                - List:
                    - Items:
                        - Ident: sum
                        - Ident: salary
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
                  - Ident: salary
                  - Raw: +
                  - Ident: payroll_tax
            - Assign:
                lvalue: gross_cost
                rvalue:
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
    count,
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
                        str: "\"USA\"",
                        start: 10,
                        end: 15,
                    },
                    inner: [],
                },
            ],
        )
        "###);
        assert_debug_snapshot!(parse_to_pest_tree(r#""USA""#, Rule::string));
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
    count,
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
