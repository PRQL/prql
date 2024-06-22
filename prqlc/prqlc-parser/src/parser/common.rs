use chumsky::prelude::*;

use crate::error::parse_error::PError;
use crate::lexer::lr::TokenKind;
use crate::parser::pr::{Annotation, Expr, ExprKind, Stmt, StmtKind, Ty, TyKind};
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

pub fn into_expr(kind: ExprKind, span: Span) -> Expr {
    Expr {
        span: Some(span),
        ..Expr::new(kind)
    }
}

pub fn into_ty(kind: TyKind, span: Span) -> Ty {
    Ty {
        span: Some(span),
        ..Ty::new(kind)
    }
}
