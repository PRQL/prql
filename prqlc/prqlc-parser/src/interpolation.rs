use chumsky::{error::Cheap, prelude::*};
use itertools::Itertools;
use prqlc_ast::expr::*;

use super::common::{into_expr, PError};
use super::lexer::*;
use super::span::ParserSpan;
use crate::Span;

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

#[test]
fn parse_interpolate() {
    use insta::assert_debug_snapshot;
    let span_base = ParserSpan::new(0, 0..0);

    assert_debug_snapshot!(
        parse("concat({a})".to_string(), span_base).unwrap(),
    @r###"
    [
        String(
            "concat(",
        ),
        Expr {
            expr: Expr {
                kind: Ident(
                    "a",
                ),
                span: Some(
                    0:8-9,
                ),
                alias: None,
            },
            format: None,
        },
        String(
            ")",
        ),
    ]
    "###);

    assert_debug_snapshot!(
        parse("print('{{hello}}')".to_string(), span_base).unwrap(),
    @r###"
    [
        String(
            "print('{hello}')",
        ),
    ]
    "###);

    assert_debug_snapshot!(
        parse("concat('{{', a, '}}')".to_string(), span_base).unwrap(),
    @r###"
    [
        String(
            "concat('{', a, '}')",
        ),
    ]
    "###);

    assert_debug_snapshot!(
        parse("concat('{{', {a}, '}}')".to_string(), span_base).unwrap(),
    @r###"
    [
        String(
            "concat('{', ",
        ),
        Expr {
            expr: Expr {
                kind: Ident(
                    "a",
                ),
                span: Some(
                    0:14-15,
                ),
                alias: None,
            },
            format: None,
        },
        String(
            ", '}')",
        ),
    ]
    "###);
}

fn parser(span_base: ParserSpan) -> impl Parser<char, Vec<InterpolateItem>, Error = Cheap<char>> {
    let expr = ident_part()
        .map_with_span(move |name, s| (name, offset_span(span_base, s)))
        .separated_by(just('.'))
        .at_least(1)
        .map(|ident_parts| {
            let mut parts = ident_parts.into_iter();

            let (first, first_span) = parts.next().unwrap();
            let mut base = Box::new(into_expr(ExprKind::Ident(first), first_span));

            for (part, span) in parts {
                let field = IndirectionKind::Name(part);
                base = Box::new(into_expr(ExprKind::Indirection { base, field }, span));
            }
            base
        })
        .then(
            just(':')
                .ignore_then(filter(|c| *c != '}').repeated().collect::<String>())
                .or_not(),
        )
        .delimited_by(just('{'), just('}'))
        .map(|(expr, format)| InterpolateItem::Expr { expr, format });

    // Convert double braces to single braces, and fail on any single braces.
    let string = (just("{{").to('{'))
        .or(just("}}").to('}'))
        .or(none_of("{}"))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(InterpolateItem::String);

    expr.or(string).repeated().then_ignore(end())
}

fn offset_span(base: ParserSpan, range: std::ops::Range<usize>) -> ParserSpan {
    ParserSpan(Span {
        start: base.0.start + range.start,
        end: base.0.start + range.end,
        source_id: base.0.source_id,
    })
}
