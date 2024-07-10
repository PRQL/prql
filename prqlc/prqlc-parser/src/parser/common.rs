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
    just(TokenKind::NewLine).ignored().labelled("new line")
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
    new_line()
        .repeated()
        .at_least(1)
        .ignore_then(
            select! {
                TokenKind::DocComment(dc) => dc,
            }
            .then_ignore(new_line().repeated())
            .repeated()
            .at_least(1)
            .collect(),
        )
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
    use insta::assert_yaml_snapshot;

    use super::*;
    use crate::parser::prepare_stream;

    fn parse_with_parser<O>(
        source: &str,
        parser: impl Parser<TokenKind, O, Error = PError>,
    ) -> Result<O, Vec<PError>> {
        let tokens = crate::lexer::lex_source(source).unwrap();
        let stream = prepare_stream(tokens.0.into_iter(), source, 0);

        let (ast, parse_errors) = parser.parse_recovery(stream);

        if !parse_errors.is_empty() {
            return Err(parse_errors);
        }
        Ok(ast.unwrap())
    }

    #[test]
    fn test_doc_comment() {
        assert_yaml_snapshot!(parse_with_parser(r#"
        #! doc comment
        #! another line

        "#, doc_comment()), @r###"
        ---
        Ok: " doc comment\n another line"
        "###);
    }
}
