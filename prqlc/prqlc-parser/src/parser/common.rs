use chumsky::prelude::*;

use super::perror::PError;
use super::pr::{Annotation, Stmt, StmtKind};
use crate::lexer::lr::TokenKind;
use crate::span::Span;

pub fn ident_part() -> impl Parser<TokenKind, String, Error = PError> + Clone {
    return select! {
        TokenKind::Ident(ident) => ident,
        TokenKind::Keyword(ident) if &ident == "module" => ident,
    }
    .map_err(|e: PError| {
        PError::expected_input_found(
            e.span(),
            [Some(TokenKind::Ident("".to_string()))],
            e.found().cloned(),
        )
    });
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
    }
}
