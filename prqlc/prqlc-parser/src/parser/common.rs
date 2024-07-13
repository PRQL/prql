use chumsky::prelude::*;

use super::perror::PError;
use super::pr::{Annotation, Stmt, StmtKind};
use crate::lexer::lr::TokenKind;
use crate::span::Span;

use super::SupportsDocComment;

pub(crate) fn ident_part() -> impl Parser<TokenKind, String, Error = PError> + Clone {
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

pub(crate) fn keyword(kw: &'static str) -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::Keyword(kw.to_string())).ignored()
}

/// Our approach to new lines is each item consumes new lines _before_ itself,
/// but not newlines after itself. This allows us to enforce new lines between
/// some items. The only place we handle new lines after an item is in the root
/// parser.
pub fn new_line() -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::NewLine)
        // Start is considered a new line, so we can enforce things start on a new
        // line while allowing them to be at the beginning of a file
        .or(just(TokenKind::Start))
        .ignored()
        .labelled("new line")
}

pub(crate) fn ctrl(char: char) -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::Control(char)).ignored()
}

pub fn into_stmt((annotations, kind): (Vec<Annotation>, StmtKind), span: Span) -> Stmt {
    Stmt {
        kind,
        span: Some(span),
        annotations,
        doc_comment: None,
    }
}

pub(crate) fn doc_comment() -> impl Parser<TokenKind, String, Error = PError> + Clone {
    // doc comments must start on a new line, so we enforce a new line (which
    // can also be a file start) before the doc comment
    //
    // TODO: we currently lose any empty newlines between doc comments;
    // eventually we want to retain them
    (new_line().repeated().at_least(1).ignore_then(select! {
        TokenKind::DocComment(dc) => dc,
    }))
    .repeated()
    .at_least(1)
    .collect()
    .map(|lines: Vec<String>| lines.join("\n"))
    .labelled("doc comment")
}

pub(crate) fn with_doc_comment<'a, P, O>(
    parser: P,
) -> impl Parser<TokenKind, O, Error = PError> + Clone + 'a
where
    P: Parser<TokenKind, O, Error = PError> + Clone + 'a,
    O: SupportsDocComment + 'a,
{
    doc_comment()
        .or_not()
        .then(parser)
        .map(|(doc_comment, inner)| inner.with_doc_comment(doc_comment))
}

/// Parse a sequence, allowing commas and new lines between items. Doesn't
/// include the surrounding delimiters.
pub(crate) fn sequence<'a, P, O>(
    parser: P,
) -> impl Parser<TokenKind, Vec<O>, Error = PError> + Clone + 'a
where
    P: Parser<TokenKind, O, Error = PError> + Clone + 'a,
    O: 'a,
{
    parser
        .separated_by(ctrl(',').then_ignore(new_line().repeated()))
        .allow_trailing()
        .padded_by(new_line().repeated())
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
