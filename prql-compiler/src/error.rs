pub use anyhow::Result;

use ariadne::{Cache, Config, Label, Report, ReportKind, Source};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::ops::{Add, Range};

use crate::parser::PestError;
use crate::utils::IntoOnly;

#[derive(Clone, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub span: Option<Span>,
    pub reason: Reason,
    pub help: Option<String>,
}

/// Location within the source file.
/// Tuples contain:
/// - line number (0-based),
/// - column number within that line (0-based),
#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub start: (usize, usize),

    pub end: (usize, usize),
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
            span: None,
            reason,
            help: None,
        }
    }

    pub fn with_help<S: Into<String>>(mut self, help: S) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMessage {
    /// Plain text of the error
    pub reason: String,
    /// A list of suggestions of how to fix the error
    pub hint: Option<String>,
    /// Character offset of error origin within a source file
    pub span: Option<Span>,

    /// Annotated code, containing cause and hints.
    pub display: Option<String>,
    /// Line and column number of error origin within a source file
    pub location: Option<SourceLocation>,
}

impl Display for ErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // https://github.com/zesterer/ariadne/issues/52
        if let Some(display) = &self.display {
            let message_without_trailing_spaces = display
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("\n");
            f.write_str(&message_without_trailing_spaces)?;
        } else {
            f.write_str(&self.reason)?;
        }
        Ok(())
    }
}

// Needed for anyhow
impl StdError for Error {}

// Needed for StdError
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self, f)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMessages {
    pub inner: Vec<ErrorMessage>,
}

impl From<ErrorMessage> for ErrorMessages {
    fn from(e: ErrorMessage) -> Self {
        ErrorMessages { inner: vec![e] }
    }
}

impl Display for ErrorMessages {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for e in &self.inner {
            Display::fmt(&e, f)?;
        }
        Ok(())
    }
}

pub fn downcast(error: anyhow::Error) -> ErrorMessages {
    let mut span = None;
    let mut hint = None;

    let error = match error.downcast::<ErrorMessages>() {
        Ok(messages) => return messages,
        Err(error) => error,
    };

    let reason = match error.downcast::<Error>() {
        Ok(error) => {
            span = error.span;
            hint = error.help;

            error.reason.message()
        }
        Err(error) => {
            match error.downcast::<PestError>() {
                Ok(error) => {
                    let range = pest::as_range(&error);
                    span = Some(Span {
                        start: range.start,
                        end: range.end,
                    });

                    pest::as_message(&error)
                }
                Err(error) => {
                    // default to basic Display
                    format!("{:#?}", error)
                }
            }
        }
    };

    ErrorMessage {
        reason,
        hint,
        span,
        display: None,
        location: None,
    }
    .into()
}

impl ErrorMessages {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    /// Computes message location and builds the pretty display.
    pub fn composed(mut self, source_id: &str, source: &str, color: bool) -> Self {
        for e in &mut self.inner {
            let source = Source::from(source);
            let cache = (source_id, source);

            e.location = e.compose_location(&cache.1);
            e.display = e.compose_display(source_id, cache, color);
        }
        self
    }
}

impl IntoOnly for ErrorMessages {
    type Item = ErrorMessage;

    fn into_only(self) -> Result<Self::Item> {
        self.inner.into_only()
    }
}

impl ErrorMessage {
    fn compose_display<'a, C>(&self, source_id: &'a str, cache: C, color: bool) -> Option<String>
    where
        C: Cache<&'a str>,
    {
        let config = Config::default().with_color(color);

        let span = Range::from(self.span?);

        let mut report = Report::build(ReportKind::Error, source_id, span.start)
            .with_config(config)
            .with_message("")
            .with_label(Label::new((source_id, span)).with_message(&self.reason));

        if let Some(hint) = &self.hint {
            report.set_help(hint);
        }

        let mut out = Vec::new();
        report.finish().write(cache, &mut out).ok()?;
        String::from_utf8(out).ok()
    }

    fn compose_location(&self, source: &Source) -> Option<SourceLocation> {
        let span = self.span?;

        let start = source.get_offset_line(span.start)?;
        let end = source.get_offset_line(span.end)?;
        Some(SourceLocation {
            start: (start.1, start.2),
            end: (end.1, end.2),
        })
    }
}

impl Reason {
    fn message(&self) -> String {
        match self {
            Reason::Simple(text) => text.clone(),
            Reason::Expected {
                who,
                expected,
                found,
            } => {
                let who = who.clone().map(|x| format!("{x} ")).unwrap_or_default();
                format!("{who}expected {expected}, but found {found}")
            }
            Reason::Unexpected { found } => format!("unexpected {found}"),
            Reason::NotFound { name, namespace } => format!("{namespace} `{name}` not found"),
        }
    }
}

mod pest {
    use pest::error::{ErrorVariant, InputLocation};
    use std::ops::Range;

    use crate::parser::{PestError, PestRule};

    pub fn as_range(error: &PestError) -> Range<usize> {
        match error.location {
            InputLocation::Pos(r) => r..r + 1,
            InputLocation::Span(r) => r.0..r.1,
        }
    }

    pub fn as_message(error: &PestError) -> String {
        match error.variant {
            ErrorVariant::ParsingError {
                ref positives,
                ref negatives,
            } => parsing_error_message(positives, negatives),
            ErrorVariant::CustomError { ref message } => message.clone(),
        }
    }

    fn parsing_error_message(positives: &[PestRule], negatives: &[PestRule]) -> String {
        match (negatives.is_empty(), positives.is_empty()) {
            (false, false) => format!(
                "unexpected {}; expected {}",
                enumerate(negatives),
                enumerate(positives)
            ),
            (false, true) => format!("unexpected {}", enumerate(negatives)),
            (true, false) => format!("expected {}", enumerate(positives)),
            (true, true) => "unknown parsing error".to_owned(),
        }
    }

    fn enumerate(rules: &[PestRule]) -> String {
        match rules.len() {
            1 => format!("{:?}", rules[0]),
            2 => format!("{:?} or {:?}", rules[0], rules[1]),
            l => {
                let separated = rules
                    .iter()
                    .take(l - 1)
                    .map(|x| format!("{:?}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}, or {:?}", separated, rules[l - 1])
            }
        }
    }
}

impl From<Span> for Range<usize> {
    fn from(a: Span) -> Self {
        a.start..a.end
    }
}

impl Add<Span> for Span {
    type Output = Span;

    fn add(self, rhs: Span) -> Span {
        Span {
            start: self.start.min(rhs.start),
            end: self.end.max(rhs.end),
        }
    }
}

pub trait WithErrorInfo {
    fn with_help<S: Into<String>>(self, help: S) -> Self;

    fn with_span(self, span: Option<Span>) -> Self;
}

impl<T> WithErrorInfo for Result<T, Error> {
    fn with_help<S: Into<String>>(self, help: S) -> Self {
        self.map_err(|e| e.with_help(help))
    }

    fn with_span(self, span: Option<Span>) -> Self {
        self.map_err(|e| e.with_span(span))
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "span-chars-{}-{}", self.start, self.end)
    }
}
