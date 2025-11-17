use chumsky;
use chumsky::error::Rich;

use crate::error::WithErrorInfo;
use crate::error::{Error, Reason};
use crate::lexer::lr::TokenKind;
use crate::span::Span;

// Helper function to convert Rich errors to our Error type
fn rich_error_to_error<T>(
    span: Span,
    reason: &chumsky::error::RichReason<T>,
    token_to_string: impl Fn(&T) -> String,
    is_whitespace_token: impl Fn(&T) -> bool,
) -> Error
where
    T: std::fmt::Debug,
{
    use chumsky::error::RichReason;

    let error = match reason {
        RichReason::ExpectedFound { expected, found } => {
            use chumsky::error::RichPattern;
            let expected_strs: Vec<String> = expected
                .iter()
                .filter(|p| {
                    // Filter out whitespace tokens unless that's all we're expecting
                    let is_whitespace = match p {
                        RichPattern::EndOfInput => true,
                        RichPattern::Token(t) => is_whitespace_token(t),
                        _ => false,
                    };
                    !is_whitespace
                        || expected.iter().all(|p| match p {
                            RichPattern::EndOfInput => true,
                            RichPattern::Token(t) => is_whitespace_token(t),
                            _ => false,
                        })
                })
                .map(|p| match p {
                    RichPattern::Token(t) => token_to_string(t),
                    RichPattern::EndOfInput => "end of input".to_string(),
                    _ => format!("{:?}", p),
                })
                .collect();

            let found_str = match found {
                Some(t) => token_to_string(t),
                None => "end of input".to_string(),
            };

            if expected_strs.is_empty() || expected_strs.len() > 10 {
                Error::new_simple(format!("unexpected {found_str}"))
            } else {
                let mut expected_strs = expected_strs;
                expected_strs.sort();

                let expected_str = match expected_strs.len() {
                    1 => expected_strs[0].clone(),
                    2 => expected_strs.join(" or "),
                    _ => {
                        let last = expected_strs.pop().unwrap();
                        format!("one of {} or {last}", expected_strs.join(", "))
                    }
                };

                match found {
                    Some(_) => Error::new(Reason::Expected {
                        who: None,
                        expected: expected_str,
                        found: found_str,
                    }),
                    None => Error::new(Reason::Simple(format!(
                        "Expected {expected_str}, but didn't find anything before the end."
                    ))),
                }
            }
        }
        RichReason::Custom(msg) => Error::new_simple(msg.to_string()),
    };

    error.with_span(Some(span))
}

impl<'a> From<Rich<'a, crate::lexer::lr::Token, Span>> for Error {
    fn from(rich: Rich<'a, crate::lexer::lr::Token, Span>) -> Error {
        rich_error_to_error(
            *rich.span(),
            rich.reason(),
            |token| format!("{}", token.kind),
            |token| matches!(token.kind, TokenKind::NewLine | TokenKind::Start),
        )
    }
}

impl<'a> From<Rich<'a, TokenKind, Span>> for Error {
    fn from(rich: Rich<'a, TokenKind, Span>) -> Error {
        rich_error_to_error(
            *rich.span(),
            rich.reason(),
            |kind| format!("{}", kind),
            |kind| matches!(kind, TokenKind::NewLine | TokenKind::Start),
        )
    }
}

#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_snapshot};

    use crate::error::{Error, WithErrorInfo};

    // Helper function to create a simple Error object
    fn simple_error(message: &str) -> Error {
        Error::new_simple(message)
    }

    #[test]
    fn test_error_messages() {
        let error1 = simple_error("test error");
        assert_snapshot!(error1.to_string(), @r#"Error { kind: Error, span: None, reason: Simple("test error"), hints: [], code: None }"#);

        let error2 = simple_error("another error").with_span(Some(crate::span::Span {
            start: 0,
            end: 5,
            source_id: 0,
        }));
        assert_debug_snapshot!(error2, @r#"
        Error {
            kind: Error,
            span: Some(
                0:0-5,
            ),
            reason: Simple(
                "another error",
            ),
            hints: [],
            code: None,
        }
        "#);
    }
}
