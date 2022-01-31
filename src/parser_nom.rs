// A previous attempt to use nom — while this is eventually a more powerful
// approach, it's harder to get started (I got stuck a few times), and so we've
// moved to using pest. We can delete this file soon, and potentially come back
// to resurrent the approach once we have a working transpiler.

#![allow(unused_imports)]
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, tag_no_case, take, take_until, take_while1};
use nom::character::complete::{multispace0, not_line_ending};
use nom::combinator::{opt, rest};
use nom::error::ParseError;
use nom::multi::separated_list0;
use nom::number::complete::be_u16;
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::IResult;
// We only use this library for this function; we could just vendor the function
// https://github.com/Geal/nom/issues/1253
use parse_hyperlinks::take_until_unbalanced;

pub type Column<'a> = &'a str;
pub type Expr<'a> = &'a str;

#[derive(Debug, PartialEq, Clone)]
pub struct Aggregate<'a> {
    pub by: Vec<Column<'a>>,
    pub calcs: Vec<Expr<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionCall<'a> {
    pub name: &'a str,
    pub positional_args: Vec<Expr<'a>>,
    pub named_args: Vec<(&'a str, Expr<'a>)>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Transformation<'a> {
    Select(Vec<Column<'a>>),
    Filter(Expr<'a>),
    Aggregate(Aggregate<'a>),
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
    alt((
        delimited(tag("("), take_until_unbalanced('(', ')'), tag(")")),
        not_line_ending,
    ))(input)
}

#[test]
fn test_parse_expr() {
    assert_eq!(parse_expr("a + b"), Ok(("", "a + b")));
    assert_eq!(parse_expr("((a + b))"), Ok(("", "(a + b)")));
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

pub fn parse_filter(input: &str) -> IResult<&str, Transformation> {
    let (remainder, expr) = preceded(tag("filter"), parse_expr)(input)?;
    Ok((remainder, Transformation::Filter(expr)))
}

#[test]
fn test_parse_filter() {
    assert_eq!(
        parse_filter("filter country = \"USA\""),
        Ok(("", Transformation::Filter(" country = \"USA\"")))
    );
    assert_eq!(
        parse_filter("filter gross_cost > 0"),
        Ok(("", Transformation::Filter(" gross_cost > 0")))
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

pub fn parse_aggregate(input: &str) -> IResult<&str, Transformation> {
    let (remainder, _) = ws(tag("aggregate"))(input)?;
    let (remainder, _) = ws(tag("by"))(remainder)?;
    let (remainder, _) = ws(tag(":"))(remainder)?;

    let (remainder, by) = parse_list(remainder)?;
    let (remainder, calcs) = parse_list(remainder)?;

    Ok((
        remainder,
        Transformation::Aggregate(Aggregate {
            by: by,
            calcs: calcs,
        }),
    ))
}

#[test]
fn test_parse_aggregate() {
    assert_eq!(
        // TODO: The current implementation of parse_list can only handle lists of
        // single words, so that's what the test case has although it is not
        // syntactically valid PRQL.
        // TODO: allow for `by` as an optional arg, in either position (either specifically in `aggregate` or a
        // more general parsing function)
        parse_aggregate("aggregate by:[title, country] [average, sum]"),
        Ok((
            "",
            Transformation::Aggregate(Aggregate {
                by: vec!["title", "country"],
                calcs: vec!["average", "sum"]
            })
        )),
    )
}
