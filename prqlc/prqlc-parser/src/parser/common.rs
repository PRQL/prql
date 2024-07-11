use chumsky::prelude::*;

use super::perror::PError;
use super::pr::{Annotation, Stmt, StmtKind};
use crate::lexer::lr::TokenKind;
use crate::span::Span;

use super::SupportsDocComment;

pub fn ident_part() -> impl Parser<TokenKind, String, Error = PError> + Clone {
    select! {
        TokenKind::Ident(ident) => ident,
        TokenKind::Keyword(ident) if &ident == "module" => ident,
    }
    .map_err(|e: PError| {
        dbg!(e.clone());
        PError::expected_input_found(
            e.span(),
            [Some(TokenKind::Ident("".to_string()))],
            e.found().cloned(),
        )
    })
}

pub fn keyword(kw: &'static str) -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::Keyword(kw.to_string())).ignored()
}

pub fn new_line() -> impl Parser<TokenKind, (), Error = PError> + Clone {
    just(TokenKind::NewLine).ignored()
}

pub fn ctrl(char: char) -> impl Parser<TokenKind, (), Error = PError> + Clone {
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

pub fn doc_comment() -> impl Parser<TokenKind, String, Error = PError> + Clone {
    // doc comments must start on a new line, so we enforce a new line before
    // the doc comment (but how to handle the start of a file?)
    //
    // TODO: we currently lose any empty newlines between doc comments;
    // eventually we want to retain them
    new_line()
        .repeated()
        .at_least(1)
        .ignore_then(select! {
            TokenKind::DocComment(dc) => dc,
        })
        .repeated()
        .at_least(1)
        .collect()
        .debug("doc_comment")
        .map(|lines: Vec<String>| lines.join("\n"))
        .labelled("doc comment")
}

pub fn with_doc_comment<'a, P, O>(
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
}
