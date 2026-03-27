use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};

#[cfg(feature = "display")]
use std::collections::HashMap;
#[cfg(feature = "display")]
use std::ops::Range;
#[cfg(feature = "display")]
use std::path::PathBuf;

#[cfg(feature = "display")]
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
                writeln!(f, "↳ Hint: {hint}")?;
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
        log::debug!("{e:#?}");
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
        for e in &mut self.inner {
            let Some(span) = e.span else {
                continue;
            };
            let Some(source_path) = sources.source_ids.get(&span.source_id) else {
                continue;
            };
            let Some(source_str) = sources.sources.get(source_path) else {
                continue;
            };

            e.location = compose_location(span, source_str);

            assert!(
                e.location.is_some(),
                "span {:?} is out of bounds of the source (len = {})",
                e.span,
                source_str.len()
            );

            #[cfg(feature = "display")]
            {
                let mut cache = FileTreeCache::new(sources);
                e.display = e.compose_display(source_path.clone(), &mut cache);
            }
        }
        self
    }
}

/// Convert a byte offset to (line, col), both 0-based.
/// Byte offsets that fall inside a multi-byte character are mapped to
/// that character's position.
fn offset_to_line_col(source: &str, offset: usize) -> Option<(usize, usize)> {
    if offset > source.len() {
        return None;
    }
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in source.char_indices() {
        if offset >= i && offset < i + ch.len_utf8() {
            return Some((line, col));
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // offset == source.len() (one past the end)
    if offset == source.len() {
        return Some((line, col));
    }
    None
}

/// Compute source location from a span without ariadne.
fn compose_location(span: Span, source: &str) -> Option<SourceLocation> {
    let start = offset_to_line_col(source, span.start)?;
    let end = offset_to_line_col(source, span.end)?;
    Some(SourceLocation { start, end })
}

#[cfg(feature = "display")]
impl ErrorMessage {
    fn compose_display(&self, source_path: PathBuf, cache: &mut FileTreeCache) -> Option<String> {
        // We always pass color to ariadne as true, and then (currently) strip later.
        let config = Config::default().with_color(true);

        // Create a span tuple with the source path and the error range
        let span = Range::from(self.span?);
        let error_span = (source_path.clone(), span.start..span.end);

        let mut report = Report::build(ReportKind::Error, error_span.clone())
            .with_config(config)
            .with_label(Label::new(error_span).with_message(&self.reason));

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
            .map(|x| crate::utils::maybe_strip_colors(x.as_str()))
    }
}

#[cfg(feature = "display")]
struct FileTreeCache<'a> {
    file_tree: &'a SourceTree,
    cache: HashMap<PathBuf, Source>,
}

#[cfg(feature = "display")]
impl<'a> FileTreeCache<'a> {
    fn new(file_tree: &'a SourceTree) -> Self {
        FileTreeCache {
            file_tree,
            cache: HashMap::new(),
        }
    }
}

#[cfg(feature = "display")]
impl Cache<PathBuf> for FileTreeCache<'_> {
    type Storage = String;
    fn fetch(&mut self, id: &PathBuf) -> Result<&Source<Self::Storage>, impl fmt::Debug> {
        let file_contents = match self.file_tree.sources.get(id) {
            Some(v) => v,
            None => return Err(format!("Unknown file `{id:?}`")),
        };

        Ok(self
            .cache
            .entry(id.clone())
            .or_insert_with(|| Source::from(file_contents.to_string())))
    }

    fn display<'b>(&self, id: &'b PathBuf) -> Option<impl fmt::Display + 'b> {
        id.as_os_str().to_str().map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_to_line_col_basic() {
        let src = "hello\nworld";
        // 'h' at offset 0
        assert_eq!(offset_to_line_col(src, 0), Some((0, 0)));
        // 'e' at offset 1
        assert_eq!(offset_to_line_col(src, 1), Some((0, 1)));
        // '\n' at offset 5
        assert_eq!(offset_to_line_col(src, 5), Some((0, 5)));
        // 'w' at offset 6
        assert_eq!(offset_to_line_col(src, 6), Some((1, 0)));
        // 'd' at offset 10
        assert_eq!(offset_to_line_col(src, 10), Some((1, 4)));
        // one past end
        assert_eq!(offset_to_line_col(src, 11), Some((1, 5)));
        // out of bounds
        assert_eq!(offset_to_line_col(src, 12), None);
    }

    #[test]
    fn offset_to_line_col_empty() {
        assert_eq!(offset_to_line_col("", 0), Some((0, 0)));
        assert_eq!(offset_to_line_col("", 1), None);
    }

    #[test]
    fn offset_to_line_col_multibyte() {
        let src = "á\nb"; // á is 2 bytes
                          // offset 0 = 'á'
        assert_eq!(offset_to_line_col(src, 0), Some((0, 0)));
        // offset 1 = mid-character (inside 'á'), maps to same char
        assert_eq!(offset_to_line_col(src, 1), Some((0, 0)));
        // offset 2 = '\n' (after the 2-byte char)
        assert_eq!(offset_to_line_col(src, 2), Some((0, 1)));
        // offset 3 = 'b'
        assert_eq!(offset_to_line_col(src, 3), Some((1, 0)));
    }

    #[test]
    fn offset_to_line_col_curly_quote() {
        // U+2019 RIGHT SINGLE QUOTATION MARK is 3 bytes in UTF-8
        let src = "S\u{2019}s";
        assert_eq!(offset_to_line_col(src, 0), Some((0, 0))); // 'S'
        assert_eq!(offset_to_line_col(src, 1), Some((0, 1))); // start of '''
        assert_eq!(offset_to_line_col(src, 2), Some((0, 1))); // mid '''
        assert_eq!(offset_to_line_col(src, 3), Some((0, 1))); // mid '''
        assert_eq!(offset_to_line_col(src, 4), Some((0, 2))); // 's'
    }
}
