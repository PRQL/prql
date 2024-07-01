use chumsky::prelude::*;

use crate::error::parse_error::PError;
use crate::lexer::lr::TokenKind;
use crate::parser::pr::{Annotation, Stmt, StmtKind};
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

pub fn aesthetic() -> impl Parser<TokenKind, TokenKind, Error = PError> + Clone {
    select! {
        // TokenKind::Comment(comment) =>         TokenKind::Comment(comment),
        // TokenKind::LineWrap(lw) =>         TokenKind::LineWrap(lw),
        TokenKind::DocComment(dc) => TokenKind::DocComment(dc),
    }
}

pub fn with_aesthetics<'a, P, O>(
    parser: P,
) -> impl Parser<TokenKind, O, Error = PError> + Clone + 'a
where
    P: Parser<TokenKind, O, Error = PError> + Clone + 'a,
    O: WithAesthetics + 'a,
{
    // We can safely remove newlines following the `aesthetics_before`, to cover
    // a case like `# foo` here:
    //
    // ```prql
    // # foo
    //
    // from bar
    // # baz
    // select artists
    // ```
    //
    // ...but not following the `aesthetics_after`; since that would eat all
    // newlines between `from_bar` and `select_artists`.
    //
    let aesthetics_before = aesthetic().then_ignore(new_line().repeated()).repeated();
    let aesthetics_after = aesthetic().separated_by(new_line());

    aesthetics_before.then(parser).then(aesthetics_after).map(
        |((aesthetics_before, inner), aesthetics_after)| {
            inner.with_aesthetics(aesthetics_before, aesthetics_after)
        },
    )
}
