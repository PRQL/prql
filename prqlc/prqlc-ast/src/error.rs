//! Error message produced by the compiler.
// Should be in some prql_common crate, but until we need that, it can reside here.

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

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
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
