#![allow(unused_imports)]
#![allow(unused_must_use)]

use insta;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
use insta::assert_yaml_snapshot;
use pest::error::Error;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

// aka Column
pub type Ident<'a> = &'a str;
pub type Items<'a> = Vec<Item<'a>>;

#[derive(Debug, PartialEq, Clone)]
pub enum Item<'a> {
    // Notes on `ListItem` are in prql.pest
    ListItem(Box<Items<'a>>),
    // Currently holds `Items`, but if we decide on this approach probably hold
    // a `ListItem` struct.
    List(Box<Items<'a>>),
    Transformation(Transformation<'a>),
    Ident(Ident<'a>),
    String(&'a str),
    Raw(&'a str),
    Assign(Assign<'a>),
    NamedArg(NamedArg<'a>),
    Query(Box<Items<'a>>),
    Pipeline(Vec<Transformation<'a>>),
    TODO(&'a str),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Transformation<'a> {
    pub name: TransformationType<'a>,
    pub args: Vec<Item<'a>>,
    pub named_args: Vec<NamedArg<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TransformationType<'a> {
    From,
    Select,
    Filter,
    Aggregate,
    Sort,
    Take,
    Custom { name: &'a str },
}

impl<'a> From<&'a str> for TransformationType<'a> {
    fn from(s: &'a str) -> Self {
        match s {
            "from" => TransformationType::From,
            "select" => TransformationType::Select,
            "filter" => TransformationType::Filter,
            "aggregate" => TransformationType::Aggregate,
            "sort" => TransformationType::Sort,
            "take" => TransformationType::Take,
            _ => TransformationType::Custom { name: s },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NamedArg<'a> {
    pub lvalue: Ident<'a>,
    pub rvalue: Box<Items<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Assign<'a> {
    pub lvalue: Ident<'a>,
    pub rvalue: Box<Items<'a>>,
}

pub fn parse<'a>(pairs: Pairs<'a, Rule>) -> Result<Items<'a>, Error<Rule>> {
    pairs
        .map(|pair| {
            Ok(match pair.as_rule() {
                Rule::list => Item::List(Box::new(parse(pair.into_inner())?)),
                Rule::list_item => Item::ListItem(Box::new(parse(pair.into_inner())?)),
                Rule::named_arg => {
                    let items_ = parse(pair.into_inner())?;
                    let lvalue = if let Item::Ident(ident) = items_[0] {
                        ident
                    } else {
                        panic!()
                    };
                    let rvalue = Box::new(items_[1..].iter().map(|x| x.clone()).collect());
                    Item::NamedArg(NamedArg { lvalue, rvalue })
                }
                Rule::assign => {
                    let items_ = parse(pair.into_inner())?;
                    let lvalue = if let Item::Ident(ident) = items_[0] {
                        ident
                    } else {
                        panic!()
                    };
                    let rvalue = Box::new(items_[1..].iter().map(|x| x.clone()).collect());
                    Item::Assign(Assign { lvalue, rvalue })
                }
                Rule::transformation => {
                    let mut items = parse(pair.into_inner())?.into_iter();
                    // TODO: these coercions are unnecessarily verbose — is
                    // there another approach? Maybe a `to_ident` method on
                    // `Item`, which fails if it's not an `Ident`? (or maybe a
                    // different design all together?)
                    let name = if let Item::Ident(ident) = items.next().unwrap() {
                        ident
                    } else {
                        panic!()
                    };
                    let mut args: Vec<Item<'a>> = vec![];
                    let mut named_args: Vec<NamedArg<'a>> = vec![];

                    for item in items {
                        match item {
                            Item::NamedArg(named_arg) => named_args.push(named_arg),
                            _ => args.push(item),
                        }
                    }
                    Item::Transformation(Transformation {
                        name: name.into(),
                        args: args,
                        named_args: named_args,
                    })
                }
                Rule::ident => Item::Ident(pair.as_str()),
                Rule::string => Item::String(pair.as_str()),
                Rule::query => Item::Query(Box::new(parse(pair.into_inner())?)),
                Rule::pipeline => Item::Pipeline({
                    parse(pair.into_inner())?
                        .into_iter()
                        .map(|x| match x {
                            Item::Transformation(transformation) => transformation,
                            _ => panic!("{:?}", x),
                        })
                        .collect()
                }),
                Rule::operator | Rule::number => Item::Raw(pair.as_str()),
                // Rule::pipeline => Item::Pipeline(Box::new(parse(pair.into_inner())?)),
                _ => (Item::TODO(pair.as_str())),
            })
        })
        .collect()
}

#[test]
fn test_parse_expr() {
    assert_debug_snapshot!(parse(
        parse_to_pest_tree(r#"country = "USA""#, Rule::expr).unwrap()
    ));
    assert_debug_snapshot!(parse(
        parse_to_pest_tree("aggregate by:[title] [sum salary]", Rule::transformation).unwrap()
    ));
    assert_debug_snapshot!(parse(
        parse_to_pest_tree(
            r#"[                                         
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
]"#,
            Rule::list,
        )
        .unwrap()
    ));
}

#[test]
fn test_parse_query() {
    assert_debug_snapshot!(parse(
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
    ));
}

pub fn parse_to_pest_tree(source: &str, rule: Rule) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(rule, source)?;
    Ok(pairs)
}

#[test]
fn test_parse_to_pest_tree() {
    assert_debug_snapshot!(parse_to_pest_tree(r#"country = "USA""#, Rule::expr));
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
