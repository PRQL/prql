use chumsky;
use chumsky::input::BorrowInput;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;

use self::pr::{Annotation, Stmt, StmtKind};
use crate::error::Error;
use crate::lexer::lr;
use crate::lexer::lr::TokenKind;
use crate::span::Span;

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

    // Use built-in Input impl for &[Token], then map_span to convert token indices to byte spans
    let input = semantic_tokens
        .as_slice()
        .map_span(|simple_span: SimpleSpan| {
            let start_idx = simple_span.start();
            let end_idx = simple_span.end();

            // Convert token indices to byte offsets in the source file
            let start = semantic_tokens
                .get(start_idx)
                .map(|t| t.span.start)
                .unwrap_or(0);
            let end = semantic_tokens
                .get(end_idx.saturating_sub(1))
                .map(|t| t.span.end)
                .unwrap_or(start);

            Span {
                start,
                end,
                source_id,
            }
        });

    let parse_result = stmt::source().parse(input);
    let (pr, parse_errors) = parse_result.into_output_errors();

    let errors = parse_errors.into_iter().map(|e| e.into()).collect();
    log::debug!("parse errors: {errors:?}");

    (pr, errors)
}

fn ident_part<'a, I>() -> impl Parser<'a, I, String, extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! {
        lr::Token { kind: TokenKind::Ident(ident), .. } => ident.clone(),
    }
}

fn keyword<'a, I>(
    kw: &'static str,
) -> impl Parser<'a, I, (), extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! {
        lr::Token { kind: TokenKind::Keyword(k), .. } if k == kw => (),
    }
}

/// Our approach to new lines is each item consumes new lines _before_ itself,
/// but not newlines after itself. This allows us to enforce new lines between
/// some items. The only place we handle new lines after an item is in the root
/// parser.
pub(crate) fn new_line<'a, I>(
) -> impl Parser<'a, I, (), extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! {
        lr::Token { kind: TokenKind::NewLine, .. } => (),
        lr::Token { kind: TokenKind::Start, .. } => (),
    }
    .labelled("new line")
}

fn ctrl<'a, I>(char: char) -> impl Parser<'a, I, (), extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! {
        lr::Token { kind: TokenKind::Control(c), .. } if *c == char => (),
    }
}

fn into_stmt((annotations, kind): (Vec<Annotation>, StmtKind), span: Span) -> Stmt {
    Stmt {
        kind,
        span: Some(span),
        annotations,
        doc_comment: None,
    }
}

fn doc_comment<'a, I>() -> impl Parser<'a, I, String, extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    // doc comments must start on a new line, so we enforce a new line (which
    // can also be a file start) before the doc comment
    //
    // TODO: we currently lose any empty newlines between doc comments;
    // eventually we want to retain or restrict them
    (new_line().repeated().at_least(1).ignore_then(select_ref! {
        lr::Token { kind: TokenKind::DocComment(dc), .. } => dc.clone(),
    }))
    .repeated()
    .at_least(1)
    .collect()
    .map(|lines: Vec<String>| lines.join("\n"))
    .labelled("doc comment")
}

fn with_doc_comment<'a, I, P, O>(
    parser: P,
) -> impl Parser<'a, I, O, extra::Err<Rich<'a, lr::Token, Span>>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    P: Parser<'a, I, O, extra::Err<Rich<'a, lr::Token, Span>>> + Clone + 'a,
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
) -> impl Parser<'a, I, Vec<O>, extra::Err<Rich<'a, lr::Token, Span>>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    P: Parser<'a, I, O, extra::Err<Rich<'a, lr::Token, Span>>> + Clone + 'a,
    O: 'a,
{
    parser
        .separated_by(ctrl(',').then_ignore(new_line().repeated()))
        .allow_trailing()
        .collect()
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

fn pipe<'a, I>() -> impl Parser<'a, I, (), extra::Err<Rich<'a, lr::Token, Span>>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
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

        let input = semantic_tokens
            .as_slice()
            .map_span(|simple_span: SimpleSpan| {
                let start_idx = simple_span.start();
                let end_idx = simple_span.end();

                let start = semantic_tokens
                    .get(start_idx)
                    .map(|t| t.span.start)
                    .unwrap_or(0);
                let end = semantic_tokens
                    .get(end_idx.saturating_sub(1))
                    .map(|t| t.span.end)
                    .unwrap_or(start);

                Span {
                    start,
                    end,
                    source_id: 0,
                }
            });

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

    // Doc comment functionality is tested in stmt.rs tests

    #[cfg(test)]
    impl SupportsDocComment for String {
        fn with_doc_comment(self, _doc_comment: Option<String>) -> Self {
            self
        }
    }
}
