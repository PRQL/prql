use ariadne::{Label, Report, ReportKind, Source};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::ops::Range;

use crate::parser::PestError;
#[derive(Debug, Clone, PartialEq, Copy, Default, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

// TODO: return this object when throwing errors
#[derive(Debug, Clone)]
pub struct Error {
    pub span: Span,
    pub reason: Reason,
    pub help: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Reason {
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
            span: Span::default(),
            reason,
            help: None,
        }
    }

    pub fn with_help<S: Into<String>>(mut self, help: S) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
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

pub fn print_error(error: anyhow::Error, source_id: &str, source: &str) {
    if let Some(error) = error.downcast_ref::<Error>() {
        let span = Range::from(error.span);

        let message = error.reason.message();

        let mut report = Report::build(ReportKind::Error, source_id, span.start)
            .with_message("")
            .with_label(Label::new((source_id, span)).with_message(&message));

        if let Some(help) = &error.help {
            report.set_help(help);
        }

        report
            .finish()
            .eprint((source_id, Source::from(source)))
            .unwrap();

        return;
    }

    if let Some(error) = error.downcast_ref::<PestError>() {
        let span = pest::as_range(error);

        Report::build(ReportKind::Error, source_id, span.start)
            .with_message("during parsing")
            .with_label(Label::new((source_id, span)).with_message(pest::as_message(error)))
            .finish()
            .eprint((source_id, Source::from(source)))
            .unwrap();
        return;
    }

    // default to basic Display
    println!("{:}", error);
}

impl Reason {
    fn message(&self) -> String {
        match self {
            Reason::Expected {
                who,
                expected,
                found,
            } => {
                let who = who.clone().map(|x| format!("`{x}` ")).unwrap_or_default();
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

pub trait WithErrorInfo {
    fn with_help<S: Into<String>>(self, help: S) -> Self;

    fn with_span(self, span: Span) -> Self;
}

impl<T> WithErrorInfo for Result<T, Error> {
    fn with_help<S: Into<String>>(self, help: S) -> Self {
        self.map_err(|e| e.with_help(help))
    }

    fn with_span(self, span: Span) -> Self {
        self.map_err(|e| e.with_span(span))
    }
}
