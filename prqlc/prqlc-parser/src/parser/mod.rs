mod common;
mod expr;
pub mod generic;
mod interpolation;
pub mod pr;
pub(crate) mod stmt;
#[cfg(test)]
mod test;
mod types;

use chumsky::{prelude::*, Stream};

use crate::error::Error;
use crate::lexer::lr::{Token, TokenKind};
use crate::parser::pr::Stmt;
use crate::span::Span;

pub fn parse_lr_to_pr(
    source: &str,
    source_id: u16,
    lr_iter: impl Iterator<Item = Token>,
) -> (Option<Vec<Stmt>>, Vec<Error>) {
    let stream = prepare_stream(lr_iter, source, source_id);
    let (pr, parse_errors) = ::chumsky::Parser::parse_recovery(&stmt::source(), stream);

    let errors = parse_errors.into_iter().map(|e| e.into()).collect();

    (pr, errors)
}

/// Convert the output of the lexer into the input of the parser. Requires
/// supplying the original source code.
fn prepare_stream(
    tokens: impl Iterator<Item = Token>,
    source: &str,
    source_id: u16,
) -> Stream<TokenKind, Span, impl Iterator<Item = (TokenKind, Span)> + Sized> {
    let tokens = tokens
        .into_iter()
        .map(move |token| (token.kind, Span::new(source_id, token.span)));
    let len = source.chars().count();
    let eoi = Span {
        start: len,
        end: len + 1,
        source_id,
    };
    Stream::from_iter(eoi, tokens)
}
