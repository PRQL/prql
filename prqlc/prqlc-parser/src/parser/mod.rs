use chumsky::{prelude::*, Stream};

use self::perror::PError;
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
    let stream = prepare_stream(lr, source_id);
    let (pr, parse_errors) = stmt::source().parse_recovery(stream);

    let errors = parse_errors.into_iter().map(|e| e.into()).collect();
    log::debug!("parse errors: {errors:?}");

    (pr, errors)
}

/// Convert the output of the lexer into the input of the parser. Requires
/// supplying the original source code.
pub(crate) fn prepare_stream<'a>(
    tokens: Vec<lr::Token>,
    source_id: u16,
) -> Stream<'a, lr::TokenKind, Span, impl Iterator<Item = (lr::TokenKind, Span)> + Sized + 'a> {
    let final_span = tokens.last().map(|t| t.span.end).unwrap_or(0);

    // We don't want comments in the AST (but we do intend to use them as part of
    // formatting)
    let semantic_tokens = tokens.into_iter().filter(|token| {
        !matches!(
            token.kind,
            lr::TokenKind::Comment(_) | lr::TokenKind::LineWrap(_)
        )
    });

    let tokens = semantic_tokens
        .into_iter()
        .map(move |token| (token.kind, Span::new(source_id, token.span)));
    let eoi = Span {
        start: final_span,
        end: final_span,
        source_id,
    };
    Stream::from_iter(eoi, tokens)
}

fn ident_part() -> impl Parser<TokenKind, String, Error = PError> + Clone {
    select! {
        TokenKind::Ident(ident) => ident,
        TokenKind::Keyword(ident) if &ident == "module" => ident,
    }
    .map_err(|e: PError| {
        PError::expected_input_found(
            e.span(),
            [Some(TokenKind::Ident("".to_string()))],
            e.found().cloned(),
        )
    })
}

fn keyword(kw: &'static str) -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::Keyword(kw.to_string())).ignored()
}

/// Our approach to new lines is each item consumes new lines _before_ itself,
/// but not newlines after itself. This allows us to enforce new lines between
/// some items. The only place we handle new lines after an item is in the root
/// parser.
pub(crate) fn new_line() -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::NewLine)
        // Start is considered a new line, so we can enforce things start on a new
        // line while allowing them to be at the beginning of a file
        .or(just(TokenKind::Start))
        .ignored()
        .labelled("new line")
}

fn ctrl(char: char) -> impl Parser<TokenKind, (), Error = PError> + Clone {
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

fn doc_comment() -> impl Parser<TokenKind, String, Error = PError> + Clone {
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

fn with_doc_comment<'a, P, O>(parser: P) -> impl Parser<TokenKind, O, Error = PError> + Clone + 'a
where
    P: Parser<TokenKind, O, Error = PError> + Clone + 'a,
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
fn sequence<'a, P, O>(parser: P) -> impl Parser<TokenKind, Vec<O>, Error = PError> + Clone + 'a
where
    P: Parser<TokenKind, O, Error = PError> + Clone + 'a,
    O: 'a,
{
    parser
        .separated_by(ctrl(',').then_ignore(new_line().repeated()))
        .allow_trailing()
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

fn pipe() -> impl Parser<TokenKind, (), Error = PError> + Clone {
    ctrl('|')
        .ignored()
        .or(new_line().repeated().at_least(1).ignored())
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;
    use crate::test::parse_with_parser;

    #[test]
    fn test_doc_comment() {
        assert_debug_snapshot!(parse_with_parser(r#"
        #! doc comment
        #! another line

        "#, doc_comment()), @r###"
        Ok(
            " doc comment\n another line",
        )
        "###);
    }

    #[test]
    fn test_doc_comment_or_not() {
        assert_debug_snapshot!(parse_with_parser(r#"hello"#, doc_comment().or_not()).unwrap(), @"None");
        assert_debug_snapshot!(parse_with_parser(r#"hello"#, doc_comment().or_not().then_ignore(new_line().repeated()).then(ident_part())).unwrap(), @r###"
        (
            None,
            "hello",
        )
        "###);
    }

    #[test]
    fn test_no_doc_comment_in_with_doc_comment() {
        impl SupportsDocComment for String {
            fn with_doc_comment(self, _doc_comment: Option<String>) -> Self {
                self
            }
        }
        assert_debug_snapshot!(parse_with_parser(r#"hello"#, with_doc_comment(new_line().ignore_then(ident_part()))).unwrap(), @r###""hello""###);
    }
}
