use chumsky::input::{SliceInput, ValueInput};
use chumsky::prelude::*;

use crate::error::{Error, WithErrorInfo};
use crate::parser::pr::*;
use crate::span::Span;

/// Parses interpolated strings
pub(crate) fn parse(string: String, span_base: Span) -> Result<Vec<InterpolateItem>, Vec<Error>> {
    let res = interpolated_parser().parse(string.as_str());

    let (output, errors) = res.into_output_errors();

    if !errors.is_empty() {
        return Err(errors
            .into_iter()
            .map(|e| {
                // Adjust span to be relative to span_base
                let span = Span {
                    start: span_base.start + e.span().start,
                    end: span_base.start + e.span().end,
                    source_id: span_base.source_id,
                };

                // Convert Rich error to our Error format
                // Custom error formatting for consistent user experience across all PRQL errors.
                // Chumsky's default format varies between versions and doesn't match our
                // "{label} expected {X}, but found {Y}" pattern used elsewhere.
                let message = {
                    // Get the label from contexts (most specific one)
                    let label = e.contexts().last().map(|(pat, _)| pat.to_string());

                    // Build expected list
                    let expected: Vec<_> = e.expected().map(|e| format!("{e}")).collect();
                    let expected_str = match expected.len() {
                        0 => String::new(),
                        1 => expected[0].clone(),
                        2 => format!("{} or {}", expected[0], expected[1]),
                        _ => format!(
                            "{}, or {}",
                            expected[..expected.len() - 1].join(", "),
                            expected.last().unwrap()
                        ),
                    };

                    // Format the found token consistently: quote actual tokens, but not "end of input"
                    let found = if let Some(f) = e.found() {
                        format!("\"{}\"", f)
                    } else {
                        "end of input".to_string()
                    };

                    if let Some(label) = label {
                        if expected_str.is_empty() {
                            format!("unexpected {found}")
                        } else {
                            format!("{label} expected {expected_str}, but found {found}")
                        }
                    } else if expected_str.is_empty() {
                        format!("unexpected {found}")
                    } else {
                        format!("expected {expected_str}, but found {found}")
                    }
                };

                Error::new_simple(message).with_span(Some(span))
            })
            .collect());
    }

    // Adjust spans in the output to be relative to span_base
    let adjusted_output = output
        .unwrap_or_default()
        .into_iter()
        .map(|item| match item {
            InterpolateItem::Expr { expr, format } => {
                let adjusted_expr = Box::new(Expr {
                    span: expr.span.map(|s| Span {
                        start: span_base.start + s.start,
                        end: span_base.start + s.end,
                        source_id: span_base.source_id,
                    }),
                    ..(*expr)
                });
                InterpolateItem::Expr {
                    expr: adjusted_expr,
                    format,
                }
            }
            InterpolateItem::String(s) => InterpolateItem::String(s),
        })
        .collect();

    Ok(adjusted_output)
}

fn interpolated_parser<'a, I>(
) -> impl Parser<'a, I, Vec<InterpolateItem>, extra::Err<Rich<'a, char, SimpleSpan>>>
where
    I: ValueInput<'a, Token = char, Span = SimpleSpan> + SliceInput<'a, Slice = &'a str>,
{
    let expr = interpolate_ident_part()
        .separated_by(just('.'))
        .at_least(1)
        .collect()
        .map(Ident::from_path)
        .map(ExprKind::Ident)
        .map_with(|kind, extra| {
            // Convert SimpleSpan to our Span type (will be adjusted in parse() function)
            let simple_span: SimpleSpan = extra.span();
            let span = Span {
                start: simple_span.start,
                end: simple_span.end,
                source_id: 0,
            };
            ExprKind::into_expr(kind, span)
        })
        .map(Box::new)
        .labelled("interpolated string variable")
        .then(
            just(':')
                .ignore_then(none_of('}').repeated().collect::<String>())
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

    expr.or(string).repeated().collect().then_ignore(end())
}

pub(crate) fn interpolate_ident_part<'a, I>(
) -> impl Parser<'a, I, String, extra::Err<Rich<'a, char, SimpleSpan>>> + Clone
where
    I: ValueInput<'a, Token = char, Span = SimpleSpan> + SliceInput<'a, Slice = &'a str>,
{
    let plain = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_')
                .repeated(),
        )
        .to_slice()
        .map(|s: &str| s.to_string())
        .labelled("interpolated string");

    let backticks = none_of('`')
        .repeated()
        .to_slice()
        .map(|s: &str| s.to_string())
        .delimited_by(just('`'), just('`'));

    plain.or(backticks.labelled("interp:backticks"))
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
