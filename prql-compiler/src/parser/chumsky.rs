#![allow(dead_code)]

use std::collections::HashMap;

use chumsky::prelude::*;
use semver::VersionReq;

use super::lexer::Token;
use crate::{ast::pl::*, Span};

pub fn source() -> impl Parser<Token, Vec<Stmt>, Error = Simple<Token>> {
    let stmt = query_def().or(main_pipeline());

    stmt.separated_by(just(Token::NewLine))
        .padded_by(just(Token::Whitespace).or(just(Token::NewLine)).repeated())
        .then_ignore(end())
}

fn main_pipeline() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    expr_call()
        .separated_by(
            just(Token::Pipe).or(just(Token::NewLine)
                .repeated()
                .at_least(1)
                .to(Token::NewLine)),
        )
        .at_least(1)
        .map(|mut exprs| {
            if exprs.len() == 1 {
                exprs.remove(0).kind
            } else {
                ExprKind::Pipeline(Pipeline { exprs })
            }
        })
        .map_with_span(into_expr)
        .map(Box::new)
        .map(StmtKind::Main)
        .map_with_span(into_stmt)
}

fn query_def() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    keyword("prql")
        .ignore_then(
            // named arg
            ident_part()
                .then_ignore(just(Token::Colon).padded_by(just(Token::Whitespace).or_not()))
                .then(expr_call())
                .repeated(),
        )
        .try_map(|args, span| {
            let mut params: HashMap<_, _> = args.into_iter().collect();

            let version = params
                .remove("version")
                .map(|v| match v.kind {
                    ExprKind::Literal(Literal::String(v)) => {
                        VersionReq::parse(&v).map_err(|e| e.to_string())
                    }
                    _ => Err("version must be a sting literal".to_string()),
                })
                .transpose()
                .map_err(|msg| Simple::custom(span, msg))?;

            let other = params
                .into_iter()
                .flat_map(|(key, value)| match value.kind {
                    ExprKind::Ident(value) => Some((key, value.to_string())),
                    _ => None,
                })
                .collect();

            Ok(StmtKind::QueryDef(QueryDef { version, other }))
        })
        .map_with_span(into_stmt)
}

pub fn expr_call() -> impl Parser<Token, Expr, Error = Simple<Token>> {
    recursive(|expr| {
        let literal = select! { Token::Literal(lit) => lit }.map(ExprKind::Literal);

        let ident_kind = ident().map(ExprKind::Ident);

        let list = just(Token::BracketL)
            .ignore_then(
                ident_part()
                    .then_ignore(just(Token::Equals).padded_by(just(Token::Whitespace).or_not()))
                    .or_not()
                    .then(expr.clone())
                    .map(|(alias, expr)| Expr { alias, ..expr })
                    .padded_by(just(Token::Whitespace).or(just(Token::NewLine)).repeated())
                    .separated_by(just(Token::Comma))
                    .allow_trailing(),
            )
            .then_ignore(just(Token::BracketR))
            .map(ExprKind::List)
            .labelled("list");

        let pipeline = just(Token::ParenthesisL)
            .ignore_then(
                expr.clone()
                    .padded_by(just(Token::Whitespace).or_not())
                    .separated_by(just(Token::Pipe).or(just(Token::NewLine))),
            )
            .then_ignore(just(Token::ParenthesisR))
            .map(|mut exprs| {
                if exprs.len() == 1 {
                    exprs.remove(0).kind
                } else {
                    ExprKind::Pipeline(Pipeline { exprs })
                }
            });

        // TODO: switch
        // TODO: s_string
        // TODO: f_string
        // TODO: range

        let term = literal
            .or(list)
            .or(pipeline)
            .or(ident_kind)
            .map_with_span(into_expr)
            .boxed();

        let unary_op = term
            .clone()
            .or(operator_unary()
                .then(term.map(Box::new))
                .map(|(op, expr)| ExprKind::Unary { op, expr })
                .map_with_span(into_expr))
            .boxed();

        let expr_mul = binary_op_parser(unary_op, operator_mul());

        let expr_add = binary_op_parser(expr_mul, operator_add());

        let expr_compare = binary_op_parser(expr_add, operator_compare());

        let expr_coalesce = binary_op_parser(expr_compare, operator_coalesce());

        let expr_logical = binary_op_parser(expr_coalesce, operator_logical());

        func_call(expr).map_with_span(into_expr).or(expr_logical)
    })
}

fn binary_op_parser<'a, Term, Op>(
    term: Term,
    op: Op,
) -> impl Parser<Token, Expr, Error = Simple<Token>> + 'a
where
    Term: Parser<Token, Expr, Error = Simple<Token>> + 'a,
    Op: Parser<Token, BinOp, Error = Simple<Token>> + 'a,
{
    let term = term.map_with_span(|e, s| (e, s)).boxed();

    (term.clone())
        .then(
            op.padded_by(just(Token::Whitespace).or_not())
                .then(term)
                .repeated(),
        )
        .foldl(|left, (op, right)| {
            let span = left.1.start..right.1.end;
            let kind = ExprKind::Binary {
                left: Box::new(left.0),
                op,
                right: Box::new(right.0),
            };
            (into_expr(kind, span.clone()), span)
        })
        .map(|(e, _)| e)
        .boxed()
}

fn func_call(
    expr: Recursive<Token, Expr, Simple<Token>>,
) -> impl Parser<Token, ExprKind, Error = Simple<Token>> + '_ {
    let name = ident()
        .boxed()
        .map(ExprKind::Ident)
        .map_with_span(into_expr);

    let named_arg = ident_part()
        .map(Some)
        .then_ignore(just(Token::Colon).padded_by(just(Token::Whitespace).or_not()))
        .then(expr.clone());

    let assign_call = ident_part()
        .then_ignore(just(Token::Equals).padded_by(just(Token::Whitespace).or_not()))
        .then(expr.clone())
        .map(|(alias, expr)| Expr {
            alias: Some(alias),
            ..expr
        });
    let positional_arg = assign_call.or(expr.clone()).map(|expr| (None, expr));

    let args = just(Token::Whitespace)
        .ignore_then(named_arg.or(positional_arg))
        .repeated()
        .at_least(1);

    name.then(args).map(|(name, args)| {
        let mut named_args = HashMap::new();
        let mut positional = Vec::new();
        for (name, arg) in args {
            if let Some(name) = name {
                named_args.insert(name, arg);
            } else {
                positional.push(arg);
            }
        }

        ExprKind::FuncCall(FuncCall {
            name: Box::new(name),
            args: positional,
            named_args,
        })
    })
}

pub fn ident() -> impl Parser<Token, Ident, Error = Simple<Token>> {
    let star = just(Token::Star).to("*".to_string());

    // TODO: !operator ~ !(keyword ~ WHITESPACE)
    //  we probably need combinator::Not, which has not been released yet.

    ident_part()
        .chain(
            just(Token::Dot)
                .ignore_then(ident_part().or(star))
                .repeated(),
        )
        .map(Ident::from_path::<String>)
}

fn operator_unary() -> impl Parser<Token, UnOp, Error = Simple<Token>> {
    select! {
        Token::UnOp(op) => op,
        Token::Plus => UnOp::Add,
        Token::Minus => UnOp::Neg,
    }
}
fn operator_mul() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    select! {
        Token::BinOp(op) if op == BinOp::Div || op == BinOp::Mod => op,
        Token::Star => BinOp::Mul,
    }
}
fn operator_add() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    select! {
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
    }
}
fn operator_compare() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    use BinOp::*;
    select! {
        Token::BinOp(op) if matches!(op, Eq | Ne | Gte | Lte | Gt | Lt) => op
    }
}
fn operator_logical() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    select! { Token::BinOp(op) if op == BinOp::And || op == BinOp::Or => op }
}
fn operator_coalesce() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    select! { Token::BinOp(op) if op == BinOp::Coalesce => op }
}

fn ident_part() -> impl Parser<Token, String, Error = Simple<Token>> {
    select! { Token::Ident(ident) => ident }
}

fn keyword(kw: &str) -> impl Parser<Token, String, Error = Simple<Token>> + '_ {
    select! { Token::Ident(ident) if ident == kw => ident }
}

fn into_stmt(kind: StmtKind, span: std::ops::Range<usize>) -> Stmt {
    Stmt {
        span: into_span(span),
        ..Stmt::from(kind)
    }
}

fn into_expr(kind: ExprKind, span: std::ops::Range<usize>) -> Expr {
    Expr {
        span: into_span(span),
        ..Expr::from(kind)
    }
}

fn into_span(span: std::ops::Range<usize>) -> Option<Span> {
    Some(Span {
        start: span.start,
        end: span.end,
    })
}
