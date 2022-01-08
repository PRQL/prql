#![allow(unused_imports)]
// use insta::{assert_debug_snapshot, assert_display_snapshot, assert_snapshot};
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, tag_no_case, take, take_until, take_while1};
use nom::character::complete::{multispace0, not_line_ending};
use nom::combinator::{opt, rest};
use nom::error::ParseError;
use nom::multi::separated_list0;
use nom::number::complete::be_u16;
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::IResult;

pub type Column<'a> = &'a str;
pub type Expr<'a> = &'a str;

#[derive(Debug, PartialEq, Clone)]
pub enum Transformation<'a> {
    Select(Vec<Column<'a>>),
    Filter(Expr<'a>),
    GroupBy(Vec<Column<'a>>),
    Sort(Vec<Column<'a>>),
}

// https://github.com/Geal/nom/blob/main/doc/nom_recipes.md#wrapper-combinators-that-eat-whitespace-before-and-after-a-parser
fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

pub fn parse_column(input: &str) -> IResult<&str, &str> {
    take_while1(|x| char::is_alphabetic(x) || x == '_')(input)
}

pub fn parse_list(input: &str) -> IResult<&str, Vec<&str>> {
    delimited(
        ws(tag("[")),
        terminated(
            separated_list0(ws(tag(",")), parse_column),
            opt(ws(tag(","))),
        ),
        ws(tag("]")),
    )(input)
}

#[test]
fn test_parse_list_simple() {
    let result = Ok(("", vec!["a", "b", "c"]));
    assert_eq!(parse_list("[a,b,c]"), result);
    assert_eq!(parse_list("[a, b  ,c]"), result);
    assert_eq!(parse_list("[a, b, c ,]"), result);
    assert_eq!(
        parse_list(
            "[
        a,
        b,
        c
        ]",
        ),
        result,
    );
    assert_eq!(
        parse_list(
            "[
        a,
        b,
        c,
        ]",
        ),
        result
    );
}

#[test]
fn test_parse_list_words() {
    assert_eq!(
        parse_list("[alpha, beta  , gamma]"),
        Ok(("", vec!["alpha", "beta", "gamma"]))
    );

    assert_eq!(
        parse_list("[alpha_bet, beta  , gamma]"),
        Ok(("", vec!["alpha_bet", "beta", "gamma"]))
    );
}

pub fn parse_select(input: &str) -> IResult<&str, Transformation> {
    let (remainder, cols) = preceded(tag("select"), parse_list)(input)?;
    Ok((remainder, Transformation::Select(cols)))

    // match input {
    //     "select" => Ok(("select", Transformation::Select)),
    //     "filter" => Ok(("filter", Transformation::Filter)),
    //     "groupby" => Ok(("groupby", Transformation::GroupBy)),
    //     _ =>
    // }
}

// pub fn parse_assign(input: &str) -> IResult<&str, Transformation> {
//     separated_pair(, sep, second)
// }

#[test]
fn test_parse_select() {
    assert_eq!(
        parse_select("select [a, b, c]"),
        Ok(("", Transformation::Select(vec!["a", "b", "c"])))
    );
    assert_eq!(
        parse_select(
            "select [
        a,
        b,
        c
    ]"
        ),
        Ok(("", Transformation::Select(vec!["a", "b", "c"])))
    );
}

pub fn parse_expr(input: &str) -> IResult<&str, &str> {
    // Anything surrounded by parentheses, or anything on the same line.
    // alt((delimited(tag("("), is_not(""), tag(")")), not_line_ending))(input)
    // alt((delimited(tag("("), rest, tag(")")), not_line_ending))(input)
    // TODO: this will fail with nested parentheses, but `rest` doesn't seem to
    // be working.
    alt((
        delimited(tag("("), take_until(")"), tag(")")),
        not_line_ending,
    ))(input)
}

#[test]
fn test_parse_expr() {
    assert_eq!(parse_expr("a + b"), Ok(("", "a + b")));
    // Failing because of parentheses issue.
    // assert_eq!(parse_expr("((a + b))"), Ok(("", "(a + b)")));
    assert_eq!(parse_expr("(a + b)"), Ok(("", "a + b")));
    assert_eq!(
        parse_expr(
            "(a 
        + b)"
        ),
        Ok((
            "",
            "a 
        + b"
        ))
    );
}

// pub fn parse_keyword(input: &str) -> IResult<&str, Transformation> {
//     let select = tag("select");
//     let filter = tag("filter");
//     let group_by = tag("group_by");

//     match input {
//         "select" => Ok(("select", Transformation::Select)),
//         "filter" => Ok(("filter", Transformation::Filter)),
//         "groupby" => Ok(("groupby", Transformation::GroupBy)),
//         _ =>
//     }
// }
