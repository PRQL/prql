use core::fmt;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use chumsky;
use chumsky::error::Rich;

use crate::error::WithErrorInfo;
use crate::error::{Error, ErrorSource, Reason};
use crate::lexer::lr::TokenKind;
use crate::span::Span;

#[derive(Clone, Debug)]
pub struct ChumError<T: Hash + Eq + Debug> {
    span: Span,
    reason: Option<String>,
    expected: HashSet<Option<T>>,
    found: Option<T>,
    label: SimpleLabel,
}

pub type PError = ChumError<TokenKind>;

impl<T: Hash + Eq + Debug> ChumError<T> {
    ///Create an error with a custom error message.
    pub fn custom<M: ToString>(span: Span, msg: M) -> Self {
        Self {
            span,
            reason: Some(msg.to_string()),
            expected: HashSet::default(),
            found: None,
            label: SimpleLabel::None,
        }
    }

    /// Returns the span that the error occurred at.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns an iterator over possible expected patterns.
    pub fn expected(&self) -> impl ExactSizeIterator<Item = &Option<T>> + '_ {
        self.expected.iter()
    }

    /// Returns the input, if any, that was found instead of an expected pattern.
    pub fn found(&self) -> Option<&T> {
        self.found.as_ref()
    }

    /// Returns the reason for the error.
    pub fn reason(&self) -> &Option<String> {
        &self.reason
    }

    /// Returns the error's label, if any.
    pub fn label(&self) -> Option<&'static str> {
        self.label.into()
    }

    /// Map the error's inputs using the given function.
    ///
    /// This can be used to unify the errors between parsing stages that operate upon two forms of input (for example,
    /// the initial lexing stage and the parsing stage in most compilers).
    pub fn map<U: Hash + Eq + Debug, F: FnMut(T) -> U>(self, mut f: F) -> ChumError<U> {
        ChumError {
            span: self.span,
            reason: self.reason,
            expected: self.expected.into_iter().map(|e| e.map(&mut f)).collect(),
            found: self.found.map(f),
            label: self.label,
        }
    }
}

// Note: We no longer implement LabelError and Error traits for ChumError.
// Instead, we use Chumsky's built-in Rich error type directly in parsers,
// and only use ChumError internally for conversion.

impl<T: Hash + Eq + Debug> PartialEq for ChumError<T> {
    fn eq(&self, other: &Self) -> bool {
        self.span == other.span
            && self.found == other.found
            && self.reason == other.reason
            && self.expected == other.expected
            && self.label == other.label
    }
}
impl<T: Hash + Eq + Debug> Eq for ChumError<T> {}

impl<T: fmt::Display + Hash + Eq + Debug> fmt::Display for ChumError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: Take `self.reason` into account

        if let Some(found) = &self.found {
            write!(f, "found {:?}", found.to_string())?;
        } else {
            write!(f, "found end of input")?;
        };

        match self.expected.len() {
            0 => {} //write!(f, " but end of input was expected")?,
            1 => write!(
                f,
                " but expected {}",
                match self.expected.iter().next().unwrap() {
                    Some(x) => format!("{:?}", x.to_string()),
                    None => "end of input".to_string(),
                },
            )?,
            _ => {
                write!(
                    f,
                    " but expected one of {}",
                    self.expected
                        .iter()
                        .map(|expected| match expected {
                            Some(x) => format!("{:?}", x.to_string()),
                            None => "end of input".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
        }

        Ok(())
    }
}

impl From<PError> for Error {
    fn from(p: PError) -> Error {
        let span = p.span();

        fn construct_parser_error(e: PError) -> Error {
            if let Some(message) = e.reason() {
                return Error::new_simple(message).with_source(ErrorSource::Parser(e));
            }

            fn token_to_string(t: Option<TokenKind>) -> String {
                t.as_ref()
                    .map(TokenKind::to_string)
                    .unwrap_or_else(|| "end of input".to_string())
            }

            let is_all_whitespace = e
                .expected()
                .all(|t| matches!(t, None | Some(TokenKind::NewLine)));
            let expected: Vec<String> = e
                .expected()
                // Only include whitespace if we're _only_ expecting whitespace,
                // otherwise we get "expected start of line or new line or }"
                // when there's no ending `}`, since a new line is allowed.
                .filter(|t| {
                    is_all_whitespace
                        || !matches!(t, None | Some(TokenKind::NewLine) | Some(TokenKind::Start))
                })
                .cloned()
                .map(token_to_string)
                .collect();

            let while_parsing = e
                .label()
                .map(|l| format!(" while parsing {l}"))
                .unwrap_or_default();

            if expected.is_empty() || expected.len() > 10 {
                let label = token_to_string(e.found().cloned());
                return Error::new_simple(format!("unexpected {label}{while_parsing}"))
                    .with_source(ErrorSource::Parser(e));
            }

            let mut expected = expected;
            expected.sort();

            let expected = match expected.len() {
                1 => expected.remove(0),
                2 => expected.join(" or "),
                _ => {
                    let last = expected.pop().unwrap();
                    format!("one of {} or {last}", expected.join(", "))
                }
            };

            match e.found() {
                Some(found) => Error::new(Reason::Expected {
                    who: e.label().map(|x| x.to_string()),
                    expected,
                    found: found.to_string(),
                })
                .with_source(ErrorSource::Parser(e)),
                // We want a friendlier message than "found end of input"...
                None => Error::new(Reason::Simple(format!(
                    "Expected {expected}, but didn't find anything before the end."
                )))
                .with_source(ErrorSource::Parser(e)),
            }
        }

        construct_parser_error(p).with_span(Some(span))
    }
}

// Convert from Chumsky's Rich error type (with lr::Token) to our Error type
impl<'a> From<Rich<'a, crate::lexer::lr::Token, Span>> for Error {
    fn from(rich: Rich<'a, crate::lexer::lr::Token, Span>) -> Error {
        use chumsky::error::RichReason;

        let span = *rich.span();

        fn token_to_string(t: &crate::lexer::lr::Token) -> String {
            t.kind.to_string()
        }

        let error = match rich.reason() {
            RichReason::ExpectedFound { expected, found } => {
                use chumsky::error::RichPattern;
                let expected_strs: Vec<String> = expected
                    .iter()
                    .filter(|p| {
                        // Filter out whitespace tokens unless that's all we're expecting
                        let is_whitespace = match p {
                            RichPattern::EndOfInput => true,
                            RichPattern::Token(t) => {
                                matches!(t.kind, TokenKind::NewLine | TokenKind::Start)
                            }
                            _ => false,
                        };
                        !is_whitespace
                            || expected.iter().all(|p| match p {
                                RichPattern::EndOfInput => true,
                                RichPattern::Token(t) => matches!(t.kind, TokenKind::NewLine),
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
                    let mut expected_sorted = expected_strs;
                    expected_sorted.sort();

                    let expected_str = match expected_sorted.len() {
                        1 => expected_sorted[0].clone(),
                        2 => expected_sorted.join(" or "),
                        _ => {
                            let last = expected_sorted.pop().unwrap();
                            format!("one of {} or {last}", expected_sorted.join(", "))
                        }
                    };

                    Error::new(Reason::Expected {
                        who: None,
                        expected: expected_str,
                        found: found_str,
                    })
                }
            }
            RichReason::Custom(msg) => Error::new_simple(msg),
        };

        error.with_span(Some(span))
    }
}

// Convert from Chumsky's Rich error type (with TokenKind) to our Error type
impl<'a> From<Rich<'a, TokenKind, Span>> for Error {
    fn from(rich: Rich<'a, TokenKind, Span>) -> Error {
        use chumsky::error::RichReason;

        let span = *rich.span();

        fn token_to_string(t: &TokenKind) -> String {
            t.to_string()
        }

        let error = match rich.reason() {
            RichReason::ExpectedFound { expected, found } => {
                use chumsky::error::RichPattern;
                let expected_strs: Vec<String> = expected
                    .iter()
                    .filter(|p| {
                        // Filter out whitespace tokens unless that's all we're expecting
                        let is_whitespace = match p {
                            RichPattern::EndOfInput => true,
                            RichPattern::Token(t) => {
                                matches!(**t, TokenKind::NewLine | TokenKind::Start)
                            }
                            _ => false,
                        };
                        !is_whitespace
                            || expected.iter().all(|p| match p {
                                RichPattern::EndOfInput => true,
                                RichPattern::Token(t) => matches!(**t, TokenKind::NewLine),
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
                    // TODO: Extract labels from context
                    Error::new_simple(format!("unexpected {found_str}"))
                } else {
                    let mut expected_sorted = expected_strs;
                    expected_sorted.sort();

                    let expected_str = match expected_sorted.len() {
                        1 => expected_sorted[0].clone(),
                        2 => expected_sorted.join(" or "),
                        _ => {
                            let last = expected_sorted.pop().unwrap();
                            format!("one of {} or {last}", expected_sorted.join(", "))
                        }
                    };

                    Error::new(Reason::Expected {
                        who: None, // TODO: Extract from context
                        expected: expected_str,
                        found: found_str,
                    })
                }
            }
            RichReason::Custom(msg) => Error::new_simple(msg),
        };

        error.with_span(Some(span))
    }
}

// Vendored from
// https://github.com/zesterer/chumsky/pull/238/files#diff-97e25e2a0e41c578875856e97b659be2719a65227c104b992e3144efa000c35eR184
// since it's private in chumsky

/// A type representing zero, one, or many labels applied to an error
#[derive(Clone, Copy, Debug, PartialEq)]
enum SimpleLabel {
    #[allow(dead_code)]
    Some(&'static str),
    None,
    #[allow(dead_code)]
    Multi,
}

impl From<SimpleLabel> for Option<&'static str> {
    fn from(label: SimpleLabel) -> Self {
        match label {
            SimpleLabel::Some(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_snapshot};

    use crate::error::{Error, Errors, Reason, WithErrorInfo};

    // Helper function to create a simple Error object
    fn create_simple_error() -> Error {
        Error::new_simple("A simple error message")
            .push_hint("take a hint")
            .with_code("E001")
    }

    #[test]
    fn display() {
        assert_snapshot!(create_simple_error(),
            @r#"Error { kind: Error, span: None, reason: Simple("A simple error message"), hints: ["take a hint"], code: Some("E001") }"#
        );

        let errors = Errors(vec![create_simple_error()]);
        assert_snapshot!(errors,
            @r#"Errors([Error { kind: Error, span: None, reason: Simple("A simple error message"), hints: ["take a hint"], code: Some("E001") }])"#
        );
        assert_debug_snapshot!(errors, @r#"
        Errors(
            [
                Error {
                    kind: Error,
                    span: None,
                    reason: Simple(
                        "A simple error message",
                    ),
                    hints: [
                        "take a hint",
                    ],
                    code: Some(
                        "E001",
                    ),
                },
            ],
        )
        "#)
    }

    #[test]
    fn test_simple_error() {
        let err = create_simple_error();
        assert_debug_snapshot!(err, @r#"
        Error {
            kind: Error,
            span: None,
            reason: Simple(
                "A simple error message",
            ),
            hints: [
                "take a hint",
            ],
            code: Some(
                "E001",
            ),
        }
        "#);
    }

    #[test]
    fn test_complex_error() {
        assert_debug_snapshot!(
        Error::new(Reason::Expected {
            who: Some("Test".to_string()),
            expected: "expected_value".to_string(),
            found: "found_value".to_string(),
        })
        .with_code("E002"), @r#"
        Error {
            kind: Error,
            span: None,
            reason: Expected {
                who: Some(
                    "Test",
                ),
                expected: "expected_value",
                found: "found_value",
            },
            hints: [],
            code: Some(
                "E002",
            ),
        }
        "#);
    }

    #[test]
    fn test_simple_error_with_result() {
        let result: Result<(), Error> = Err(Error::new_simple("A simple error message"))
            .with_hints(vec!["Take a hint"])
            .push_hint("Take another hint")
            .with_code("E001");
        assert_debug_snapshot!(result, @r#"
        Err(
            Error {
                kind: Error,
                span: None,
                reason: Simple(
                    "A simple error message",
                ),
                hints: [
                    "Take a hint",
                    "Take another hint",
                ],
                code: Some(
                    "E001",
                ),
            },
        )
        "#);
    }
}
