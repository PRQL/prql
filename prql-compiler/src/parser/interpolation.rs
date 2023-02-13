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
                        .map(|ch| ch.map(|c| Token::Control(c.to_string())))
                        .collect_vec(),
                    err.found().cloned().map(|c| Token::Control(c.to_string())),
                )
            })
            .collect_vec()),
    }
}

fn parser(span_offset: usize) -> impl Parser<char, Vec<InterpolateItem>, Error = Simple<char>> {
    let expr = ident_part()
        .separated_by(just('.'))
        .delimited_by(just('{'), just('}'))
        .map(Ident::from_path)
        .map(ExprKind::Ident)
        .map_with_span(move |e, s| into_expr(e, offset_span(s, span_offset)))
        .map(Box::new)
        .map(InterpolateItem::Expr);

    let escape = (just("{{").to('{'))
        .chain(just("}}").not().repeated())
        .chain(just("}}").to('}'))
        .collect::<String>()
        .map(InterpolateItem::String);

    let string = none_of('{')
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(InterpolateItem::String);

    escape.or(expr).or(string).repeated().then_ignore(end())
}

fn offset_span(mut span: std::ops::Range<usize>, span_offset: usize) -> std::ops::Range<usize> {
    span.start += span_offset;
    span.end += span_offset;
    span
}
