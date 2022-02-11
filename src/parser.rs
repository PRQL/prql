use pest::error::Error;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

// Idents are generally columns
pub type Ident = String;
pub type Items = Vec<Item>;
pub type Idents = Vec<Ident>;
pub type Pipeline = Vec<Transformation>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Transformation(Transformation),
    Ident(Ident),
    String(String),
    Raw(String),
    Assign(Assign),
    NamedArg(NamedArg),
    Query(Items),
    Pipeline(Pipeline),
    // Holds Item-s directly if a list entry is a single item, otherwise holds
    // Item::Items. This is less verbose than always having Item::Items.
    List(Items),
    // In some cases, as as lists, we need a container for multiple items to
    // discriminate them from, e.g. a series of Idents. `[a, b]` vs `[a b]`.
    Items(Items),
    Idents(Idents),
    Function(Function),
    // Anything not yet implemented.
    TODO(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Transformation {
    pub name: TransformationType,
    pub args: Items,
    pub named_args: Vec<NamedArg>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Hash, Eq, Deserialize)]
pub enum TransformationType {
    From,
    Select,
    Filter,
    Derive,
    Aggregate,
    Sort,
    Take,
    Custom { name: String },
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub body: Items,
}

impl From<&str> for TransformationType {
    fn from(s: &str) -> Self {
        match s {
            "from" => TransformationType::From,
            "select" => TransformationType::Select,
            "filter" => TransformationType::Filter,
            "derive" => TransformationType::Derive,
            "aggregate" => TransformationType::Aggregate,
            "sort" => TransformationType::Sort,
            "take" => TransformationType::Take,
            _ => TransformationType::Custom { name: s.to_owned() },
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedArg {
    pub lvalue: Ident,
    pub rvalue: Items,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Assign {
    pub lvalue: Ident,
    pub rvalue: Items,
}

impl Item {
    pub fn as_ident(&self) -> Ident {
        // TODO: Make this into a Result when we've got better error handling.
        // We could expand these with (but it will add lots of methods...)
        // https://crates.io/crates/enum-as-inner?
        if let Item::Ident(ident) = self {
            ident.to_owned()
        } else {
            panic!("Expected Item::Ident, got {:?}", self)
        }
    }
}

pub fn parse(pairs: Pairs<Rule>) -> Result<Items, Error<Rule>> {
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
                        lvalue: lvalue.as_ident(),
                        rvalue: rvalue.to_vec(),
                    })
                }
                Rule::assign => {
                    let parsed = parse(pair.into_inner())?;
                    let (lvalue, rvalue) = parsed.split_first().unwrap();

                    Item::Assign(Assign {
                        lvalue: lvalue.as_ident(),
                        rvalue: rvalue.to_vec(),
                    })
                }
                Rule::transformation => {
                    let parsed = parse(pair.into_inner())?;
                    let (name, all_args) = parsed.split_first().unwrap();

                    let mut args: Vec<Item> = vec![];
                    let mut named_args: Vec<NamedArg> = vec![];

                    for arg in all_args {
                        match arg {
                            // We seem to need the clones...
                            Item::NamedArg(named_arg) => named_args.push(named_arg.clone()),
                            _ => args.push(arg.clone()),
                        }
                    }
                    Item::Transformation(Transformation {
                        name: name.as_ident().as_str().into(),
                        args,
                        named_args,
                    })
                }
                Rule::function => {
                    let mut items = parse(pair.into_inner())?.into_iter();
                    let mut name_and_params = if let Item::Idents(idents) = items.next().unwrap() {
                        idents
                    } else {
                        unreachable!()
                    };

                    let name = name_and_params.remove(0);

                    let body = if let Item::Items(sub_items) = items.next().unwrap() {
                        sub_items
                    } else {
                        unreachable!()
                    };

                    Item::Function(Function {
                        name,
                        args: name_and_params,
                        body,
                    })
                }
                Rule::ident => Item::Ident(pair.as_str().to_string()),
                Rule::idents => Item::Idents(
                    parse(pair.into_inner())?
                        .into_iter()
                        .map(|x| x.as_ident())
                        .collect(),
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
                // Rule::pipeline => Item::Pipeline(Box::new(parse(pair.into_inner())?)),
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
            name: Aggregate
            args:
              - List:
                  - Items:
                      - Ident: sum
                      - Ident: salary
            named_args:
              - lvalue: by
                rvalue:
                  - List:
                      - Ident: title
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
    }

    #[test]
    fn test_parse_query() {
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
    "#,
                Rule::query,
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
        assert_debug_snapshot!(parse_to_pest_tree(r#"[a, b, c,]"#, Rule::list));
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
    }

    #[test]
    fn test_parse_to_pest_tree_query() {
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"
    from employees
    select [a, b]
    "#,
            Rule::query
        ));
        assert_debug_snapshot!(parse_to_pest_tree(
            r#"
    from employees
    filter country = "USA"
    "#,
            Rule::query
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
    "#,
            Rule::query
        ));
    }
}
