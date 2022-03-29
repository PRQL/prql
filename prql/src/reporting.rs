use ariadne::{Label, Report, ReportKind, Source};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};

use crate::parser::PestError;
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

// TODO: return this object when throwing errors
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SimpleError {
    Unexpected { expected: String, span: Span },
}

// Needed for anyhow
impl StdError for SimpleError {}

// Needed for StdError
impl Display for SimpleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self, f)
    }
}

pub fn print_error(error: anyhow::Error, source_id: &str, source: &str) {
    if let Some(error) = error.downcast_ref::<SimpleError>() {
        println!("{error:?}");
        return;
    }

    if let Some(error) = error.downcast_ref::<PestError>() {
        let span = pest::range(error);

        Report::build(ReportKind::Error, source_id, span.start)
            .with_message("during parsing")
            .with_label(Label::new((source_id, span)).with_message(pest::message(error)))
            .finish()
            .eprint((source_id, Source::from(source)))
            .unwrap();
        return;
    }

    // default to basic Display
    println!("{:}", error);
}

mod pest {
    use pest::error::{ErrorVariant, InputLocation};
    use std::ops::Range;

    use crate::parser::{PestError, PestRule};

    pub fn range(error: &PestError) -> Range<usize> {
        match error.location {
            InputLocation::Pos(r) => r..r + 1,
            InputLocation::Span(r) => r.0..r.1,
        }
    }

    pub fn message(error: &PestError) -> String {
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
