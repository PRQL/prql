#![allow(unused_imports)]
#![allow(unused_must_use)]

use insta;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
use insta::assert_yaml_snapshot;
use pest;

use pest::error::Error;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

pub fn parse_query(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::query, source)?;
    Ok(pairs)
}

pub fn parse_transform(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::transform, source)?;
    Ok(pairs)
}

pub fn parse_expr(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::expr, source)?;
    Ok(pairs)
}

pub fn parse_string(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::string, source)?;
    Ok(pairs)
}

pub fn parse_idents(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::idents, source)?;
    Ok(pairs)
}

#[test]
fn test_parse_expr() {
    assert_debug_snapshot!(parse_expr(r#"country = "USA""#));
}

#[test]
fn test_parse_string() {
    assert_debug_snapshot!(parse_string(r#""USA""#));
}

#[test]
fn test_parse_transform() {
    assert_debug_snapshot!(parse_transform("select [a, b, c]"));
    assert_debug_snapshot!(parse_transform(r#"    from employees"#));
    assert_debug_snapshot!(parse_transform(r#"    filter country = "USA""#));
}

#[test]
fn test_parse_query() {
    assert_debug_snapshot!(parse_query(
        r#"
    from employees
    select [a, b]
    "#
    ));
    // TODO: The comment doesn't seem to be handled correctly.
    assert_debug_snapshot!(parse_query(
        r#"
    from employees
    filter country = "USA"
    "#
    ));
}

#[test]
fn test_parse_idents() {
    assert_debug_snapshot!(parse_idents(r#"country = "USA""#));
}
