use chumsky::{error::Cheap, prelude::*};
use itertools::Itertools;

use crate::ast::pl::*;
use crate::Span;

use super::common::{into_expr, PError};
use super::lexer::*;

/// Parses interpolated strings
pub fn parse(string: String, span_base: Span) -> Result<Vec<InterpolateItem>, Vec<PError>> {
    let res = parser(span_base).parse(string);

    match res {
        Ok(items) => Ok(items),
        Err(errors) => Err(errors
            .into_iter()
            .map(|err| Simple::expected_input_found(offset_span(span_base, err.span()), None, None))
            .collect_vec()),
    }
}

fn parser(span_base: Span) -> impl Parser<char, Vec<InterpolateItem>, Error = Cheap<char>> {
    let expr = ident_part()
        .separated_by(just('.'))
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

fn offset_span(base: Span, range: std::ops::Range<usize>) -> Span {
    Span {
        start: base.start + range.start,
        end: base.start + range.end,
        source_id: base.source_id,
    }
}
