#![allow(dead_code)]

use chumsky::prelude::*;

use crate::ast::pl::*;

fn str(chars: &str) -> impl Parser<char, (), Error = Simple<char>> + '_ {
    just(chars).to(())
}

fn operator() -> impl Parser<char, (), Error = Simple<char>> {
    operator_binary().to(()).or(operator_unary().to(()))
}

fn operator_binary() -> impl Parser<char, BinOp, Error = Simple<char>> {
    operator_mul()
        .or(operator_add())
        .or(operator_compare())
        .or(operator_logical())
        .or(operator_coalesce())
}
fn operator_unary() -> impl Parser<char, UnOp, Error = Simple<char>> {
    just('-')
        .to(UnOp::Neg)
        .or(just('+').to(UnOp::Add))
        .or(just('!').to(UnOp::Not))
        .or(str("==").to(UnOp::EqSelf))
}
fn operator_mul() -> impl Parser<char, BinOp, Error = Simple<char>> {
    (just('*').to(BinOp::Mul))
        .or(just('/').to(BinOp::Div))
        .or(just('%').to(BinOp::Mod))
}
fn operator_add() -> impl Parser<char, BinOp, Error = Simple<char>> {
    just('+').to(BinOp::Add).or(just('-').to(BinOp::Sub))
}
fn operator_compare() -> impl Parser<char, BinOp, Error = Simple<char>> {
    str("==")
        .to(BinOp::Eq)
        .or(str("!=").to(BinOp::Ne))
        .or(str(">=").to(BinOp::Gte))
        .or(str("<=").to(BinOp::Lte))
        .or(str(">").to(BinOp::Gt))
        .or(str("<").to(BinOp::Lt))
}
fn operator_logical() -> impl Parser<char, BinOp, Error = Simple<char>> {
    (just("and").to(BinOp::Add))
        .or(just("or").to(BinOp::Or))
        .then_ignore(text::whitespace())
}
fn operator_coalesce() -> impl Parser<char, BinOp, Error = Simple<char>> {
    just("??").map(|_| BinOp::Coalesce)
}

fn ident_part() -> impl Parser<char, String, Error = Simple<char>> {
    let plain = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_' || *c == '$')
        .map(Some)
        .chain::<char, Vec<_>, _>(
            filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated(),
        )
        .collect();

    let backticks = just('`')
        .ignore_then(filter(|c| *c != '`').repeated())
        .then_ignore(just('`'))
        .collect::<String>();

    plain.or(backticks)
}

pub fn ident() -> impl Parser<char, Ident, Error = Simple<char>> {
    let star = just('*').map(|c| c.to_string());

    // TODO: !operator ~ !(keyword ~ WHITESPACE)
    //  we probably need combinator::Not, which has not been released yet.

    ident_part()
        .chain(just('.').ignore_then(ident_part().or(star)).repeated())
        .map(Ident::from_path::<String>)
}
