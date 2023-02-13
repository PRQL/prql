use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;

use super::common::*;
use super::lexer::{InterpolItem, Token};
use crate::ast::pl::*;

pub fn expr_call() -> impl Parser<Token, Expr, Error = Simple<Token>> {
    func_call(expr())
}

pub fn expr() -> impl Parser<Token, Expr, Error = Simple<Token>> + Clone {
    recursive(|expr| {
        let literal = select! { Token::Literal(lit) => lit }.map(ExprKind::Literal);

        let ident_kind = ident().map(ExprKind::Ident);

        let list = ctrl("[")
            .ignore_then(
                ident_part()
                    .then_ignore(ctrl("=").padded_by(whitespace().or_not()))
                    .or_not()
                    .then(func_call(expr.clone()))
                    .map(|(alias, expr)| Expr { alias, ..expr })
                    .padded_by(whitespace().or(new_line()).repeated())
                    .separated_by(ctrl(","))
                    .allow_trailing(),
            )
            .then_ignore(whitespace().or(new_line()).repeated())
            .then_ignore(ctrl("]"))
            .map(ExprKind::List)
            .labelled("list");

        let pipeline = ctrl("(")
            .ignore_then(pipeline(func_call(expr)))
            .then_ignore(ctrl(")"));

        let s_string = select! { Token::Interpolation('s', string) => string }
            .map(|s| {
                s.into_iter()
                    .map(|i| match i {
                        InterpolItem::String(s) => InterpolateItem::String(s),
                        InterpolItem::Expr(s) => InterpolateItem::String(s),
                    })
                    .collect_vec()
            })
            .map(ExprKind::SString);

        let f_string = select! { Token::Interpolation('f', string) => string }
            .map(|s| {
                s.into_iter()
                    .map(|i| match i {
                        InterpolItem::String(s) => InterpolateItem::String(s),
                        InterpolItem::Expr(s) => InterpolateItem::String(s),
                    })
                    .collect_vec()
            })
            .map(ExprKind::FString);

        // TODO: switch
        // TODO: range

        let term = literal
            .or(list)
            .or(pipeline)
            .or(s_string)
            .or(f_string)
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

        binary_op_parser(expr_coalesce, operator_logical())
    })
}

pub fn pipeline<E>(expr: E) -> impl Parser<Token, ExprKind, Error = Simple<Token>>
where
    E: Parser<Token, Expr, Error = Simple<Token>>,
{
    // expr is a param, so it can be either a normal expr() or
    // a recursive expr called from within expr()

    (new_line().or(whitespace()).repeated())
        .ignore_then(
            expr.padded_by(whitespace().or_not())
                .separated_by(ctrl("|").or(new_line().repeated().at_least(1).to(())))
                .at_least(1)
                .map(|mut exprs| {
                    if exprs.len() == 1 {
                        exprs.remove(0).kind
                    } else {
                        ExprKind::Pipeline(Pipeline { exprs })
                    }
                }),
        )
        .then_ignore(new_line().or(whitespace()).repeated())
        .labelled("pipeline")
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
        .then(op.padded_by(whitespace().or_not()).then(term).repeated())
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

fn func_call<E>(expr: E) -> impl Parser<Token, Expr, Error = Simple<Token>>
where
    E: Parser<Token, Expr, Error = Simple<Token>> + Clone,
{
    let func = expr.clone();

    let named_arg = ident_part()
        .map(Some)
        .then_ignore(ctrl(":").padded_by(whitespace().or_not()))
        .then(expr.clone());

    let assign_call = ident_part()
        .then_ignore(ctrl("=").padded_by(whitespace().or_not()))
        .then(expr.clone())
        .map(|(alias, expr)| Expr {
            alias: Some(alias),
            ..expr
        });
    let positional_arg = assign_call.or(expr).map(|expr| (None, expr));

    let args = whitespace()
        .ignore_then(named_arg.or(positional_arg))
        .repeated();

    func.then(args)
        .map(|(name, args)| {
            if args.is_empty() {
                return name.kind;
            }

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
        .map_with_span(into_expr)
        .padded_by(whitespace().or_not())
        .labelled("function call")
}

pub fn ident() -> impl Parser<Token, Ident, Error = Simple<Token>> {
    let star = ctrl("*").to("*".to_string());

    // TODO: !operator ~ !(keyword ~ WHITESPACE)
    //  we probably need combinator::Not, which has not been released yet.

    ident_part()
        .chain(ctrl(".").ignore_then(ident_part().or(star)).repeated())
        .map(Ident::from_path::<String>)
        .labelled("identifier")
}

fn operator_unary() -> impl Parser<Token, UnOp, Error = Simple<Token>> {
    (ctrl("+").to(UnOp::Add))
        .or(ctrl("-").to(UnOp::Neg))
        .or(ctrl("!").to(UnOp::Not))
        .or(ctrl("==").to(UnOp::EqSelf))
}
fn operator_mul() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl("*").to(BinOp::Mul))
        .or(ctrl("/").to(BinOp::Div))
        .or(ctrl("%").to(BinOp::Mod))
}
fn operator_add() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl("+").to(BinOp::Add)).or(ctrl("-").to(BinOp::Sub))
}
fn operator_compare() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl("==").to(BinOp::Eq))
        .or(ctrl("!=").to(BinOp::Ne))
        .or(ctrl("<=").to(BinOp::Lte))
        .or(ctrl(">=").to(BinOp::Gte))
        .or(ctrl("<").to(BinOp::Lt))
        .or(ctrl(">").to(BinOp::Gt))
}
fn operator_logical() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl("and").to(BinOp::And)).or(ctrl("or").to(BinOp::Or))
}
fn operator_coalesce() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    ctrl("??").to(BinOp::Coalesce)
}
