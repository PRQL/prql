#![allow(unused_imports)]
#![allow(unused_must_use)]

use pest;

use pest::error::Error;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "prql.pest"]
pub struct PrqlParser;

pub fn parse(source: &str) -> Result<Pairs<Rule>, Error<Rule>> {
    let pairs = PrqlParser::parse(Rule::program, source)?;
    Ok(pairs)
}

pub fn main() {
    dbg!(parse("select [a, b, c]"));
}
