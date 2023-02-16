use chumsky::prelude::*;
use itertools::Itertools;

use crate::ast::pl::*;

use super::{common::into_expr, lexer::*};

/// Parses interpolated strings
pub fn parse(
    string: String,
    span_offset: usize,
) -> Result<Vec<InterpolateItem>, Vec<Simple<Token>>> {
    let res = parser(span_offset).parse(string);

    match res {
        Ok(items) => Ok(items),
        Err(errors) => Err(errors
            .into_iter()
            .map(|err| {
                Simple::expected_input_found(
                    offset_span(err.span(), span_offset),
                    err.expected()
                        .cloned()
                        .map(|ch| ch.map(|c| Token::Control))
                        .collect_vec(),
                    err.found().cloned().map(|c| Token::Control),
                )
            })
            .collect_vec()),
    }
}

fn parser(span_offset: usize) -> impl Parser<char, Vec<InterpolateItem>, Error = Simple<char>> {
    let expr = ident_part()
        .separated_by(just('.'))
        .delimited_by(just('{'), just('}'))
        .ignored();

    let escape = (just("{{").to('{'))
        .chain(just("}}").not().repeated())
        .chain(just("}}").to('}'))
        .ignored();

    let string = none_of('{').repeated().at_least(1).ignored();

    escape
        .or(expr)
        .or(string)
        .repeated()
        .then_ignore(end())
        .to(vec![])
}

fn offset_span(mut span: std::ops::Range<usize>, span_offset: usize) -> std::ops::Range<usize> {
    span.start += span_offset;
    span.end += span_offset;
    span
}
