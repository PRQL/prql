use chumsky::prelude::*;
use itertools::Itertools;

use crate::lexer::lr::{Literal, TokenKind};
use crate::parser::perror::{ChumError, PError};
use crate::parser::pr::*;
use crate::span::{string_stream, Span};

/// Parses interpolated strings
pub(crate) fn parse(string: String, span_base: Span) -> Result<Vec<InterpolateItem>, Vec<PError>> {
    let prepped_stream = string_stream(string, span_base);

    let res = interpolated_parser().parse(prepped_stream);

    match res {
        Ok(items) => {
            log::trace!("interpolated string ok: {:?}", items);
            Ok(items)
        }
        Err(errors) => Err(errors
            .into_iter()
            .map(|err| {
                log::debug!("interpolated string error (lex inside parse): {:?}", err);
                err.map(|c| TokenKind::Literal(Literal::String(c.to_string())))
            })
            .collect_vec()),
    }
}

fn interpolated_parser() -> impl Parser<char, Vec<InterpolateItem>, Error = ChumError<char>> {
    let expr = interpolate_ident_part()
        .separated_by(just('.'))
        .at_least(1)
        .map(Ident::from_path)
        .map(ExprKind::Ident)
        .map_with_span(ExprKind::into_expr)
        .map(Box::new)
        .labelled("interpolated string variable")
        .then(
            just(':')
                .ignore_then(filter(|c| *c != '}').repeated().collect::<String>())
                .or_not(),
        )
        .delimited_by(just('{'), just('}'))
        .map(|(expr, format)| InterpolateItem::Expr { expr, format });

    // Convert double braces to single braces, and fail on any single braces.
    let string = just("{{")
        .to('{')
        .or(just("}}").to('}'))
        .or(none_of("{}"))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(InterpolateItem::String);

    expr.or(string).repeated().then_ignore(end())
}

pub(crate) fn interpolate_ident_part() -> impl Parser<char, String, Error = ChumError<char>> + Clone
{
    let plain = filter(|c: &char| c.is_alphabetic() || *c == '_')
        .chain(filter(|c: &char| c.is_alphanumeric() || *c == '_').repeated())
        .labelled("interpolated string");

    let backticks = none_of('`').repeated().delimited_by(just('`'), just('`'));

    plain.or(backticks.labelled("interp:backticks")).collect()
}

#[test]
fn parse_interpolate() {
    use insta::assert_debug_snapshot;
    let span_base = Span::new(0, 0..0);

    assert_debug_snapshot!(
        parse("concat({a})".to_string(), span_base).unwrap(),
    @r#"
    [
        String(
            "concat(",
        ),
        Expr {
            expr: Expr {
                kind: Ident(
                    [
                        "a",
                    ],
                ),
                span: Some(
                    0:8-9,
                ),
                alias: None,
                doc_comment: None,
            },
            format: None,
        },
        String(
            ")",
        ),
    ]
    "#);

    assert_debug_snapshot!(
        parse("print('{{hello}}')".to_string(), span_base).unwrap(),
    @r#"
    [
        String(
            "print('{hello}')",
        ),
    ]
    "#);

    assert_debug_snapshot!(
        parse("concat('{{', a, '}}')".to_string(), span_base).unwrap(),
    @r#"
    [
        String(
            "concat('{', a, '}')",
        ),
    ]
    "#);

    assert_debug_snapshot!(
        parse("concat('{{', {a}, '}}')".to_string(), span_base).unwrap(),
    @r#"
    [
        String(
            "concat('{', ",
        ),
        Expr {
            expr: Expr {
                kind: Ident(
                    [
                        "a",
                    ],
                ),
                span: Some(
                    0:14-15,
                ),
                alias: None,
                doc_comment: None,
            },
            format: None,
        },
        String(
            ", '}')",
        ),
    ]
    "#);
}
