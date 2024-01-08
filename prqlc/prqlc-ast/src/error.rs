//! Error message produced by the compiler.

use std::fmt::Debug;

use serde::Serialize;

use crate::Span;

#[derive(Debug, Clone)]
pub struct Error {
    /// Message kind. Currently only Error is implemented.
    pub kind: MessageKind,
    pub span: Option<Span>,
    pub reason: Reason,
    pub hints: Vec<String>,
    pub code: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct Errors(pub Vec<Error>);

/// Compile message kind. Currently only Error is implemented.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MessageKind {
    Error,
    Warning,
    Lint,
}

#[derive(Debug, Clone)]
pub enum Reason {
    Simple(String),
    Expected {
        who: Option<String>,
        expected: String,
        found: String,
    },
    Unexpected {
        found: String,
    },
    NotFound {
        name: String,
        namespace: String,
    },
}

impl Error {
    pub fn new(reason: Reason) -> Self {
        Error {
            kind: MessageKind::Error,
            span: None,
            reason,
            hints: Vec::new(),
            code: None,
        }
    }

    pub fn new_simple<S: ToString>(reason: S) -> Self {
        Error::new(Reason::Simple(reason.to_string()))
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::Simple(text) => f.write_str(text),
            Reason::Expected {
                who,
                expected,
                found,
            } => {
                if let Some(who) = who {
                    write!(f, "{who} ")?;
                }
                write!(f, "expected {expected}, but found {found}")
            }
            Reason::Unexpected { found } => write!(f, "unexpected {found}"),
            Reason::NotFound { name, namespace } => write!(f, "{namespace} `{name}` not found"),
        }
    }
}

// Needed for anyhow
impl std::error::Error for Error {}

// Needed for anyhow
impl std::error::Error for Errors {}

// Needed for StdError
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

// Needed for StdError
impl std::fmt::Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

pub trait WithErrorInfo: Sized {
    fn push_hint<S: Into<String>>(self, hint: S) -> Self;

    fn with_hints<S: Into<String>, I: IntoIterator<Item = S>>(self, hints: I) -> Self;

    fn with_span(self, span: Option<Span>) -> Self;
    fn with_code(self, code: &'static str) -> Self;
}

impl WithErrorInfo for Error {
    fn with_hints<S: Into<String>, I: IntoIterator<Item = S>>(mut self, hints: I) -> Self {
        self.hints = hints.into_iter().map(|x| x.into()).collect();
        self
    }

    fn with_span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    fn push_hint<S: Into<String>>(mut self, hint: S) -> Self {
        self.hints.push(hint.into());
        self
    }

    fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }
}

#[cfg(feature = "anyhow")]
impl WithErrorInfo for anyhow::Error {
    fn push_hint<S: Into<String>>(self, hint: S) -> Self {
        self.downcast_ref::<Error>()
            .map(|e| e.clone().push_hint(hint).into())
            .unwrap_or(self)
    }

    fn with_hints<S: Into<String>, I: IntoIterator<Item = S>>(self, hints: I) -> Self {
        self.downcast_ref::<Error>()
            .map(|e| e.clone().with_hints(hints).into())
            .unwrap_or(self)
    }

    // Add a span of an expression onto the error. We need this implementation
    // because we often pass `anyhow::Error`, and still want to try adding a
    // span. So we need to try downcasting it to our error type first, and that
    // fails, we return the original error.
    fn with_span(self, span: Option<Span>) -> Self {
        self.downcast_ref::<Error>()
            .map(|e| e.clone().with_span(span).into())
            .unwrap_or(self)
    }
    fn with_code(self, code: &'static str) -> Self {
        self.downcast_ref::<Error>()
            .map(|e| e.clone().with_code(code).into())
            .unwrap_or(self)
    }
}

impl<T, E: WithErrorInfo> WithErrorInfo for Result<T, E> {
    fn with_hints<S: Into<String>, I: IntoIterator<Item = S>>(self, hints: I) -> Self {
        self.map_err(|e| e.with_hints(hints))
    }

    fn with_span(self, span: Option<Span>) -> Self {
        self.map_err(|e| e.with_span(span))
    }

    fn with_code(self, code: &'static str) -> Self {
        self.map_err(|e| e.with_code(code))
    }

    fn push_hint<S: Into<String>>(self, hint: S) -> Self {
        self.map_err(|e| e.push_hint(hint))
    }
}
