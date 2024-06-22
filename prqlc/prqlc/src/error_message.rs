use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::ops::Range;
use std::path::PathBuf;
use std::{collections::HashMap, io::stderr};

use anstream::adapter::strip_str;
use ariadne::{Cache, Config, Label, Report, ReportKind, Source};
use serde::Serialize;

use crate::Span;
use crate::{Error, Errors, MessageKind, SourceTree};

#[derive(Clone, Serialize)]
pub struct ErrorMessage {
    /// Message kind. Currently only Error is implemented.
    pub kind: MessageKind,
    /// Machine-readable identifier of the error
    pub code: Option<String>,
    /// Plain text of the error
    pub reason: String,
    /// A list of suggestions of how to fix the error
    pub hints: Vec<String>,
    /// Character offset of error origin within a source file
    pub span: Option<Span>,
    /// Annotated code, containing cause and hints.
    pub display: Option<String>,
    /// Line and column number of error origin within a source file
    pub location: Option<SourceLocation>,
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

impl Display for ErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // https://github.com/zesterer/ariadne/issues/52
        if let Some(display) = &self.display {
            let message_without_trailing_spaces = display
                .split('\n')
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            f.write_str(&message_without_trailing_spaces)?;
        } else {
            let code = (self.code.as_ref())
                .map(|c| format!("[{c}] "))
                .unwrap_or_default();

            writeln!(f, "{}Error: {}", code, &self.reason)?;
            for hint in &self.hints {
                // TODO: consider alternative formatting for hints.
                writeln!(f, "↳ Hint: {}", hint)?;
            }
        }
        Ok(())
    }
}

impl Debug for ErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self, f)
    }
}

impl From<Error> for ErrorMessage {
    fn from(e: Error) -> Self {
        log::debug!("{:#?}", e);
        ErrorMessage {
            code: e.code.map(str::to_string),
            kind: e.kind,
            reason: e.reason.to_string(),
            hints: e.hints,
            span: e.span,
            display: None,
            location: None,
        }
    }
}

impl From<Vec<ErrorMessage>> for ErrorMessages {
    fn from(errors: Vec<ErrorMessage>) -> Self {
        ErrorMessages { inner: errors }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMessages {
    pub inner: Vec<ErrorMessage>,
}
impl StdError for ErrorMessages {}

impl From<ErrorMessage> for ErrorMessages {
    fn from(e: ErrorMessage) -> Self {
        ErrorMessages { inner: vec![e] }
    }
}

impl From<Error> for ErrorMessages {
    fn from(e: Error) -> Self {
        ErrorMessages {
            inner: vec![ErrorMessage::from(e)],
        }
    }
}

impl From<Errors> for ErrorMessages {
    fn from(errs: Errors) -> Self {
        ErrorMessages {
            inner: errs.0.into_iter().map(ErrorMessage::from).collect(),
        }
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

impl ErrorMessages {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    /// Computes message location and builds the pretty display.
    pub fn composed(mut self, sources: &SourceTree) -> Self {
        let mut cache = FileTreeCache::new(sources);

        for e in &mut self.inner {
            let Some(span) = e.span else {
                continue;
            };
            let Some(source_path) = sources.source_ids.get(&span.source_id) else {
                continue;
            };

            let Ok(source) = cache.fetch(source_path) else {
                continue;
            };
            e.location = e.compose_location(source);

            assert!(
                e.location.is_some(),
                "span {:?} is out of bounds of the source (len = {})",
                e.span,
                source.len()
            );
            e.display = e.compose_display(source_path.clone(), &mut cache);
        }
        self
    }
}

impl ErrorMessage {
    fn compose_display(&self, source_path: PathBuf, cache: &mut FileTreeCache) -> Option<String> {
        // We always pass color to ariadne as true, and then (currently) strip later.
        let config = Config::default().with_color(true);

        let span = Range::from(self.span?);

        let mut report = Report::build(ReportKind::Error, source_path.clone(), span.start)
            .with_config(config)
            .with_label(Label::new((source_path, span)).with_message(&self.reason));

        if let Some(code) = &self.code {
            report = report.with_code(code);
        }

        // I don't know how to set multiple hints...
        if !self.hints.is_empty() {
            report.set_help(&self.hints[0]);
        }
        if self.hints.len() > 1 {
            report.set_note(&self.hints[1]);
        }
        if self.hints.len() > 2 {
            report.set_message(&self.hints[2]);
        }

        let mut out = Vec::new();
        report.finish().write(cache, &mut out).ok()?;
        String::from_utf8(out)
            .ok()
            .map(|x| maybe_strip_colors(x.as_str()))
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

fn should_use_color() -> bool {
    match anstream::AutoStream::choice(&stderr()) {
        anstream::ColorChoice::Auto => true,
        anstream::ColorChoice::Always => true,
        anstream::ColorChoice::AlwaysAnsi => true,
        anstream::ColorChoice::Never => false,
    }
}

/// Strip colors, for external libraries which don't yet strip themselves, and
/// for insta snapshot tests. This will respond to environment variables such as
/// `CLI_COLOR`.
pub(crate) fn maybe_strip_colors(s: &str) -> String {
    if !should_use_color() {
        strip_str(s).to_string()
    } else {
        s.to_string()
    }
}

struct FileTreeCache<'a> {
    file_tree: &'a SourceTree,
    cache: HashMap<PathBuf, Source>,
}
impl<'a> FileTreeCache<'a> {
    fn new(file_tree: &'a SourceTree) -> Self {
        FileTreeCache {
            file_tree,
            cache: HashMap::new(),
        }
    }
}

impl<'a> Cache<PathBuf> for FileTreeCache<'a> {
    type Storage = String;
    fn fetch(&mut self, id: &PathBuf) -> Result<&Source, Box<dyn fmt::Debug + '_>> {
        let file_contents = match self.file_tree.sources.get(id) {
            Some(v) => v,
            None => return Err(Box::new(format!("Unknown file `{id:?}`"))),
        };

        Ok(self
            .cache
            .entry(id.clone())
            .or_insert_with(|| Source::from(file_contents.to_string())))
    }

    fn display<'b>(&self, id: &'b PathBuf) -> Option<Box<dyn fmt::Display + 'b>> {
        match id.as_os_str().to_str() {
            Some(s) => Some(Box::new(s)),
            None => None,
        }
    }
}
