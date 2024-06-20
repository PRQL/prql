use chumsky::prelude::*;

use crate::error::parse_error::PError;
use crate::lexer::lr::TokenKind;
use crate::parser::pr::{Annotation, Expr, ExprKind, Stmt, StmtKind, Ty, TyKind};
use crate::parser::WithAesthetics;
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
        aesthetics_before: Vec::new(),
        aesthetics_after: Vec::new(),
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

pub fn aesthetic() -> impl Parser<TokenKind, TokenKind, Error = PError> + Clone {
    select! {
        TokenKind::Comment(comment) =>         TokenKind::Comment(comment),
        TokenKind::LineWrap(lw) =>         TokenKind::LineWrap(lw),
        TokenKind::DocComment(dc) => TokenKind::DocComment(dc),
    }
}

pub fn with_aesthetics<P, O>(parser: P) -> impl Parser<TokenKind, O, Error = PError> + Clone
where
    P: Parser<TokenKind, O, Error = PError> + Clone,
    O: WithAesthetics,
{
    // We can have newlines between the aesthetics and the actual token to
    // cover a case like `# foo` here:
    //
    // ```prql
    // # foo
    //
    // from bar
    // # baz
    // select artists
    // ```
    //
    // ...but not after the aesthetics after the token; since we don't want
    // to eat the newline after `from bar`
    //
    let aesthetics_before = aesthetic().then_ignore(new_line().repeated()).repeated();
    let aesthetics_after = aesthetic().separated_by(new_line());

    aesthetics_before.then(parser).then(aesthetics_after).map(
        |((aesthetics_before, inner), aesthetics_after)| {
            inner.with_aesthetics(aesthetics_before, aesthetics_after)
        },
    )
}
