use chumsky;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

use crate::error::{Error, WithErrorInfo};
use crate::parser::pr::*;
use crate::span::Span;

/// Parses interpolated strings
pub(crate) fn parse(string: String, span_base: Span) -> Result<Vec<InterpolateItem>, Vec<Error>> {
    let res = interpolated_parser().parse(string.as_str());

    let (output, errors) = res.into_output_errors();

    if !errors.is_empty() {
        // Convert Rich errors to our Error type
        return Err(errors
            .into_iter()
            .map(|e| {
                use chumsky::error::RichReason;

                // Adjust span to be relative to span_base
                let span = Span {
                    start: span_base.start + e.span().start,
                    end: span_base.start + e.span().end,
                    source_id: span_base.source_id,
                };

                // Format the error message properly based on the reason
                let message = match e.reason() {
                    RichReason::ExpectedFound { expected, found } => {
                        use chumsky::error::RichPattern;

                        // First check if we have a label - that gives the best context
                        let has_label = expected.iter().any(|p| matches!(p, RichPattern::Label(_)));

                        let expected_strs: Vec<String> = expected
                            .iter()
                            .filter_map(|p| match p {
                                RichPattern::Token(c) => Some(format!("{:?}", c)),
                                RichPattern::EndOfInput => Some("end of input".to_string()),
                                RichPattern::Label(l) => {
                                    // If it's the only pattern, use it; otherwise filter it out
                                    // since we'll use it as a prefix
                                    if expected.len() == 1 {
                                        Some(l.to_string())
                                    } else {
                                        None
                                    }
                                }
                                // Don't include these generic patterns in the list
                                RichPattern::Identifier(_)
                                | RichPattern::Any
                                | RichPattern::SomethingElse => None,
                            })
                            .collect();

                        let found_str = match found {
                            Some(c) => format!("{:?}", c),
                            None => "end of input".to_string(),
                        };

                        // Build the message with label as prefix if present
                        let label_prefix = if has_label && expected.len() > 1 {
                            expected
                                .iter()
                                .find_map(|p| match p {
                                    RichPattern::Label(l) => Some(format!("{} ", l)),
                                    _ => None,
                                })
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };

                        if expected_strs.is_empty() {
                            format!("{}unexpected {}", label_prefix, found_str)
                        } else {
                            let expected_str = match expected_strs.len() {
                                1 => expected_strs[0].clone(),
                                2 => expected_strs.join(" or "),
                                _ => {
                                    let mut sorted = expected_strs;
                                    sorted.sort();
                                    let last = sorted.pop().unwrap();
                                    format!("one of {} or {}", sorted.join(", "), last)
                                }
                            };
                            format!(
                                "{}expected {}, but found {}",
                                label_prefix, expected_str, found_str
                            )
                        }
                    }
                    RichReason::Custom(msg) => msg.to_string(),
                };

                Error::new_simple(message).with_span(Some(span))
            })
            .collect());
    }

    Ok(output.unwrap_or_default())
}

fn interpolated_parser<'a, I>(
) -> impl Parser<'a, I, Vec<InterpolateItem>, extra::Err<Rich<'a, char, SimpleSpan>>>
where
    I: Input<'a, Token = char, Span = SimpleSpan> + ValueInput<'a>,
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
                .ignore_then(
                    any()
                        .filter(|c: &char| *c != '}')
                        .repeated()
                        .collect::<String>(),
                )
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
    I: Input<'a, Token = char, Span = SimpleSpan> + ValueInput<'a>,
{
    let plain = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_')
                .repeated()
                .collect(),
        )
        .map(|(first, rest): (char, Vec<char>)| {
            let mut s = String::new();
            s.push(first);
            s.extend(rest);
            s
        })
        .labelled("interpolated string");

    let backticks = none_of('`')
        .repeated()
        .collect::<String>()
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
