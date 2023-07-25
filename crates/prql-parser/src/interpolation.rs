use chumsky::{error::Cheap, prelude::*};
use itertools::Itertools;
use prql_ast::expr::*;

use crate::Span;

use super::common::{into_expr, PError};
use super::lexer::*;
use super::span::ParserSpan;

/// Parses interpolated strings
pub fn parse(string: String, span_base: ParserSpan) -> Result<Vec<InterpolateItem>, Vec<PError>> {
    let res = parser(span_base).parse(string);

    match res {
        Ok(items) => Ok(items),
        Err(errors) => Err(errors
            .into_iter()
            .map(|err| Simple::expected_input_found(offset_span(span_base, err.span()), None, None))
            .collect_vec()),
    }
}

fn parser(span_base: ParserSpan) -> impl Parser<char, Vec<InterpolateItem>, Error = Cheap<char>> {
    let expr = ident_part()
        .separated_by(just('.'))
        .at_least(1)
        .then(
            just(':')
                .ignore_then(filter(|c| *c != '}').repeated().collect::<String>())
                .or_not(),
        )
        .delimited_by(just('{'), just('}'))
        .map_with_span(move |(ident, format), s| {
            let ident = ExprKind::Ident(Ident::from_path(ident));
            let expr = into_expr(ident, offset_span(span_base, s));
            let expr = Box::new(expr);

            InterpolateItem::Expr { expr, format }
        });

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

fn offset_span(base: ParserSpan, range: std::ops::Range<usize>) -> ParserSpan {
    ParserSpan(Span {
        start: base.0.start + range.start,
        end: base.0.start + range.end,
        source_id: base.0.source_id,
    })
}
