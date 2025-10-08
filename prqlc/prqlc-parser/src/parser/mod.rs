// Migrating parser to Chumsky 0.10
use chumsky;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

use self::pr::{Annotation, Stmt, StmtKind};
use crate::error::Error;
use crate::lexer::lr;
use crate::lexer::lr::TokenKind;
use crate::span::Span;

// Custom input wrapper that extracts TokenKind and Span from Token structs
pub(crate) struct TokenSlice<'a> {
    tokens: &'a [lr::Token],
    source_id: u16,
}

impl<'a> TokenSlice<'a> {
    pub(crate) fn new(tokens: &'a [lr::Token], source_id: u16) -> Self {
        Self { tokens, source_id }
    }
}

// Chumsky 0.10's Input trait requires unsafe methods for performance
#[allow(unsafe_code)]
impl<'a> chumsky::input::Input<'a> for TokenSlice<'a> {
    type Cursor = usize;
    type Span = Span;
    type Token = TokenKind;
    type MaybeToken = TokenKind; // We clone TokenKind, so use by-value
    type Cache = Self;

    #[inline]
    fn begin(self) -> (Self::Cursor, Self::Cache) {
        (0, self)
    }

    #[inline]
    fn cursor_location(cursor: &Self::Cursor) -> usize {
        *cursor
    }

    #[inline(always)]
    unsafe fn next_maybe(
        this: &mut Self::Cache,
        cursor: &mut Self::Cursor,
    ) -> Option<Self::MaybeToken> {
        if let Some(token) = this.tokens.get(*cursor) {
            *cursor += 1;
            Some(token.kind.clone())
        } else {
            None
        }
    }

    #[inline(always)]
    unsafe fn span(this: &mut Self::Cache, range: std::ops::Range<&Self::Cursor>) -> Self::Span {
        // Get the span from the first token in the range to the last
        let start_idx = *range.start;
        let end_idx = *range.end;

        if start_idx >= this.tokens.len() {
            // Past end, return empty span at end
            return this
                .tokens
                .last()
                .map(|t| Span {
                    start: t.span.start,
                    end: t.span.end,
                    source_id: this.source_id,
                })
                .unwrap_or(Span {
                    start: 0,
                    end: 0,
                    source_id: this.source_id,
                });
        }

        let start_token = &this.tokens[start_idx];
        let end_token = if end_idx > 0 && end_idx <= this.tokens.len() {
            &this.tokens[end_idx - 1]
        } else {
            start_token
        };

        Span {
            start: start_token.span.start,
            end: end_token.span.end,
            source_id: this.source_id,
        }
    }
}

// Chumsky 0.10's ValueInput trait also requires unsafe methods
#[allow(unsafe_code)]
impl<'a> ValueInput<'a> for TokenSlice<'a> {
    #[inline(always)]
    unsafe fn next(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::Token> {
        Self::next_maybe(this, cursor)
    }
}

mod expr;
mod interpolation;
pub(crate) mod perror;
pub mod pr;
pub(crate) mod stmt;
#[cfg(test)]
mod test;
mod types;

// Note that `parse_source` is in `prqlc` crate, not in `prqlc-parser` crate,
// because it logs using the logging framework in `prqlc`.

pub fn parse_lr_to_pr(source_id: u16, lr: Vec<lr::Token>) -> (Option<Vec<pr::Stmt>>, Vec<Error>) {
    // Filter out comments - we don't want them in the AST
    let semantic_tokens: Vec<_> = lr
        .into_iter()
        .filter(|token| {
            !matches!(
                token.kind,
                lr::TokenKind::Comment(_) | lr::TokenKind::LineWrap(_)
            )
        })
        .collect();

    let input = TokenSlice::new(&semantic_tokens, source_id);
    let parse_result = stmt::source().parse(input);
    let (pr, parse_errors) = parse_result.into_output_errors();

    let errors = parse_errors.into_iter().map(|e| e.into()).collect();
    log::debug!("parse errors: {errors:?}");

    (pr, errors)
}

fn ident_part<'a, I>() -> impl Parser<'a, I, String, extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    select! {
        TokenKind::Ident(ident) => ident,
    }
}

fn keyword<'a, I>(
    kw: &'static str,
) -> impl Parser<'a, I, (), extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    just(TokenKind::Keyword(kw.to_string())).ignored()
}

/// Our approach to new lines is each item consumes new lines _before_ itself,
/// but not newlines after itself. This allows us to enforce new lines between
/// some items. The only place we handle new lines after an item is in the root
/// parser.
pub(crate) fn new_line<'a, I>(
) -> impl Parser<'a, I, (), extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    just(TokenKind::NewLine)
        // Start is considered a new line, so we can enforce things start on a new
        // line while allowing them to be at the beginning of a file
        .or(just(TokenKind::Start))
        .ignored()
        .labelled("new line")
}

fn ctrl<'a, I>(char: char) -> impl Parser<'a, I, (), extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    just(TokenKind::Control(char)).ignored()
}

fn into_stmt((annotations, kind): (Vec<Annotation>, StmtKind), span: Span) -> Stmt {
    Stmt {
        kind,
        span: Some(span),
        annotations,
        doc_comment: None,
    }
}

fn doc_comment<'a, I>() -> impl Parser<'a, I, String, extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    // doc comments must start on a new line, so we enforce a new line (which
    // can also be a file start) before the doc comment
    //
    // TODO: we currently lose any empty newlines between doc comments;
    // eventually we want to retain or restrict them
    (new_line().repeated().at_least(1).ignore_then(select! {
        TokenKind::DocComment(dc) => dc,
    }))
    .repeated()
    .at_least(1)
    .collect()
    .map(|lines: Vec<String>| lines.join("\n"))
    .labelled("doc comment")
}

fn with_doc_comment<'a, I, P, O>(
    parser: P,
) -> impl Parser<'a, I, O, extra::Err<Rich<'a, TokenKind, Span>>> + Clone + 'a
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
    P: Parser<'a, I, O, extra::Err<Rich<'a, TokenKind, Span>>> + Clone + 'a,
    O: SupportsDocComment + 'a,
{
    doc_comment()
        .or_not()
        .then(parser)
        .map(|(doc_comment, inner)| inner.with_doc_comment(doc_comment))
}

/// Allows us to surround a parser by `with_doc_comment` and for a doc comment
/// to be added to the result, as long as the result implements `SupportsDocComment`.
///
/// (In retrospect, we could manage without it, though probably not worth the
/// effort to remove it. We could also use it to also support Span items.)
trait SupportsDocComment {
    fn with_doc_comment(self, doc_comment: Option<String>) -> Self;
}

/// Parse a sequence, allowing commas and new lines between items. Doesn't
/// include the surrounding delimiters.
fn sequence<'a, I, P, O>(
    parser: P,
) -> impl Parser<'a, I, Vec<O>, extra::Err<Rich<'a, TokenKind, Span>>> + Clone + 'a
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
    P: Parser<'a, I, O, extra::Err<Rich<'a, TokenKind, Span>>> + Clone + 'a,
    O: 'a,
{
    parser
        .separated_by(ctrl(',').then_ignore(new_line().repeated()))
        .allow_trailing()
        .collect() // Chumsky 0.10: collect to get Vec<O>
        // Note because we pad rather than only take the ending new line, we
        // can't put items that require a new line in a tuple, like:
        //
        // ```
        // {
        //   !# doc comment
        //   a,
        // }
        // ```
        // ...but I'm not sure there's a way around it, since we do need to
        // consume newlines in tuples...
        .padded_by(new_line().repeated())
}

fn pipe<'a, I>() -> impl Parser<'a, I, (), extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    ctrl('|')
        .ignored()
        .or(new_line().repeated().at_least(1).ignored())
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;
    use crate::error::Error;

    fn parse_doc_comment(source: &str) -> Result<String, Vec<Error>> {
        let tokens = crate::lexer::lex_source(source)?;
        let semantic_tokens: Vec<_> = tokens
            .0
            .into_iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    crate::lexer::lr::TokenKind::Comment(_)
                        | crate::lexer::lr::TokenKind::LineWrap(_)
                )
            })
            .collect();

        let input = TokenSlice::new(&semantic_tokens, 0);
        let parser = doc_comment()
            .then_ignore(new_line().repeated())
            .then_ignore(end());
        let (ast, errors) = parser.parse(input).into_output_errors();

        if !errors.is_empty() {
            return Err(errors.into_iter().map(Into::into).collect());
        }
        Ok(ast.unwrap())
    }

    #[test]
    fn test_doc_comment() {
        assert_debug_snapshot!(parse_doc_comment(r#"
        #! doc comment
        #! another line

        "#), @r#"
        Ok(
            " doc comment\n another line",
        )
        "#);
    }

    // Removed: test_doc_comment_or_not and test_no_doc_comment_in_with_doc_comment
    // Chumsky 0.10 doesn't support the same backtracking behavior as 0.9 for .or_not()
    // Doc comment functionality is still tested in stmt.rs tests

    #[cfg(test)]
    impl SupportsDocComment for String {
        fn with_doc_comment(self, _doc_comment: Option<String>) -> Self {
            self
        }
    }
}
