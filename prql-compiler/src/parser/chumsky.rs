use std::{collections::HashMap, str::FromStr};

use chumsky::prelude::*;
use itertools::Itertools;
use semver::VersionReq;

use super::lexer::{InterpolItem, Token};
use crate::{ast::pl::*, Span};

pub fn source() -> impl Parser<Token, Vec<Stmt>, Error = Simple<Token>> {
    query_def()
        .or_not()
        .chain(
            var_def()
                .or(function_def())
                .separated_by(new_line().or(whitespace()).repeated()),
        )
        .chain(main_pipeline().or_not())
        .then_ignore(end())
        .labelled("source file")
}

fn main_pipeline() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    pipeline(expr_call())
        .map_with_span(into_expr)
        .map(Box::new)
        .map(StmtKind::Main)
        .map_with_span(into_stmt)
        .labelled("main pipeline")
}

fn query_def() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    keyword("prql")
        .ignore_then(
            // named arg
            ident_part()
                .then_ignore(ctrl(":").padded_by(whitespace().or_not()))
                .then(expr_call())
                .repeated(),
        )
        .then_ignore(whitespace().or_not().then(new_line()))
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
        .labelled("query header")
}

fn var_def() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    keyword("let")
        .ignore_then(whitespace())
        .ignore_then(ident_part())
        .then_ignore(ctrl("=").padded_by(whitespace().or_not()))
        .then(expr_call().map(Box::new))
        .map(|(name, value)| VarDef { name, value })
        .map(StmtKind::VarDef)
        .map_with_span(into_stmt)
        .labelled("variable definition")
}

fn function_def() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    keyword("func")
        .ignore_then(whitespace())
        .ignore_then(
            // func name
            ident_part()
                .then_ignore(whitespace().or_not())
                .then(type_expr().or_not()),
        )
        .then(
            // params
            ident_part()
                .then_ignore(whitespace().or_not())
                .then(type_expr().or_not())
                .then_ignore(whitespace().or_not())
                .then(
                    ctrl(":")
                        .ignore_then(whitespace().or_not())
                        .ignore_then(expr())
                        .or_not(),
                )
                .repeated(),
        )
        .then_ignore(whitespace().or_not())
        .then_ignore(ctrl("->"))
        .then(expr_call().map(Box::new))
        .then_ignore(whitespace().or_not())
        .then_ignore(new_line())
        .map(|(((name, return_ty), params), body)| {
            let (pos, nam) = params
                .into_iter()
                .map(|((name, ty), default_value)| FuncParam {
                    name,
                    ty,
                    default_value,
                })
                .partition(|p| p.default_value.is_none());

            FuncDef {
                name,
                positional_params: pos,
                named_params: nam,
                body,
                return_ty,
            }
        })
        .map(StmtKind::FuncDef)
        .map_with_span(into_stmt)
        .labelled("function definition")
}

pub fn type_expr() -> impl Parser<Token, Ty, Error = Simple<Token>> {
    recursive(|type_expr| {
        let type_term = ident_part().then(type_expr.or_not()).map(|(name, param)| {
            let ty = match TyLit::from_str(&name) {
                Ok(t) => Ty::from(t),
                Err(_) if name == "table" => Ty::Table(Frame::default()),
                Err(_) => {
                    eprintln!("named type: {}", name);
                    Ty::Named(name.to_string())
                }
            };

            if let Some(param) = param {
                Ty::Parameterized(Box::new(ty), Box::new(param))
            } else {
                ty
            }
        });

        ctrl("<")
            .ignore_then(type_term.separated_by(ctrl("|").padded_by(whitespace().or_not())))
            .then_ignore(ctrl(">"))
            .map(|mut terms| {
                if terms.len() == 1 {
                    terms.remove(0)
                } else {
                    Ty::AnyOf(terms)
                }
            })
    })
    .labelled("type expression")
}

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

fn pipeline<E>(expr: E) -> impl Parser<Token, ExprKind, Error = Simple<Token>>
where
    E: Parser<Token, Expr, Error = Simple<Token>>,
{
    // expr is a param, so I can be either a normal expr() or
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
    let name = expr.clone();

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
    let positional_arg = assign_call.or(expr.clone()).map(|expr| (None, expr));

    let args = whitespace()
        .ignore_then(named_arg.or(positional_arg))
        .repeated();

    name.then(args)
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

fn ident_part() -> impl Parser<Token, String, Error = Simple<Token>> {
    select! { Token::Ident(ident) => ident }
}

fn keyword(kw: &str) -> impl Parser<Token, (), Error = Simple<Token>> + '_ {
    select! { Token::Ident(ident) if ident == kw => () }
}

fn whitespace() -> impl Parser<Token, (), Error = Simple<Token>> + Clone {
    just(Token::Whitespace).repeated().at_least(1).to(())
}

fn new_line() -> impl Parser<Token, (), Error = Simple<Token>> + Clone {
    just(Token::NewLine).to(())
}

fn ctrl(chars: &'static str) -> impl Parser<Token, (), Error = Simple<Token>> {
    select! { Token::Control(str) if str == chars => () }
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
