use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;

use super::interpolation;
use crate::error::parse_error::PError;
use crate::lexer::lr::{Literal, TokenKind};
use crate::parser::common::{ctrl, ident_part, keyword, new_line};
use crate::parser::pr::Ident;
use crate::parser::pr::*;
use crate::parser::pr::{BinOp, UnOp};
use crate::parser::types::type_expr;
use crate::span::Span;

pub fn expr_call() -> impl Parser<TokenKind, Expr, Error = PError> + Clone {
    let expr = expr();

    lambda_func(expr.clone()).or(func_call(expr))
}

pub fn expr() -> impl Parser<TokenKind, Expr, Error = PError> + Clone {
    recursive(|expr| {
        let literal = select! { TokenKind::Literal(lit) => ExprKind::Literal(lit) };

        let ident_kind = ident_part().map(ExprKind::Ident);

        let internal = keyword("internal")
            .ignore_then(ident())
            .map(|x| x.to_string())
            .map(ExprKind::Internal);

        let nested_expr = pipeline(lambda_func(expr.clone()).or(func_call(expr.clone()))).boxed();

        let tuple = ident_part()
            .then_ignore(ctrl('='))
            .or_not()
            .then(nested_expr.clone())
            .map(|(alias, mut expr)| {
                expr.alias = alias.or(expr.alias);
                expr
            })
            .padded_by(new_line().repeated())
            .separated_by(ctrl(','))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('{'), ctrl('}'))
            .recover_with(nested_delimiters(
                TokenKind::Control('{'),
                TokenKind::Control('}'),
                [
                    (TokenKind::Control('{'), TokenKind::Control('}')),
                    (TokenKind::Control('('), TokenKind::Control(')')),
                    (TokenKind::Control('['), TokenKind::Control(']')),
                ],
                |_| vec![],
            ))
            .map(ExprKind::Tuple)
            .labelled("tuple");

        let array = nested_expr
            .clone()
            .padded_by(new_line().repeated())
            .separated_by(ctrl(','))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('['), ctrl(']'))
            .recover_with(nested_delimiters(
                TokenKind::Control('['),
                TokenKind::Control(']'),
                [
                    (TokenKind::Control('{'), TokenKind::Control('}')),
                    (TokenKind::Control('('), TokenKind::Control(')')),
                    (TokenKind::Control('['), TokenKind::Control(']')),
                ],
                |_| vec![],
            ))
            .map(ExprKind::Array)
            .labelled("array");

        let pipeline =
            nested_expr
                .delimited_by(ctrl('('), ctrl(')'))
                .recover_with(nested_delimiters(
                    TokenKind::Control('('),
                    TokenKind::Control(')'),
                    [
                        (TokenKind::Control('['), TokenKind::Control(']')),
                        (TokenKind::Control('('), TokenKind::Control(')')),
                    ],
                    |_| Expr::new(ExprKind::Literal(Literal::Null)),
                ));

        let interpolation = select! {
            TokenKind::Interpolation('s', string) => (ExprKind::SString as fn(_) -> _, string),
            TokenKind::Interpolation('f', string) => (ExprKind::FString as fn(_) -> _, string),
        }
        .validate(|(finish, string), span: Span, emit| {
            match interpolation::parse(string, span + 2) {
                Ok(items) => finish(items),
                Err(errors) => {
                    for err in errors {
                        emit(err)
                    }
                    finish(vec![])
                }
            }
        })
        .labelled("interpolated string");

        let case = keyword("case")
            .ignore_then(
                func_call(expr.clone())
                    .map(Box::new)
                    .then_ignore(just(TokenKind::ArrowFat))
                    .then(func_call(expr.clone()).map(Box::new))
                    .map(|(condition, value)| SwitchCase { condition, value })
                    .padded_by(new_line().repeated())
                    .separated_by(ctrl(','))
                    .allow_trailing()
                    .then_ignore(new_line().repeated())
                    .delimited_by(ctrl('['), ctrl(']')),
            )
            .map(ExprKind::Case);

        let param = select! { TokenKind::Param(id) => ExprKind::Param(id) };

        let term = choice((
            literal,
            internal,
            tuple,
            array,
            interpolation,
            ident_kind,
            case,
            param,
        ))
        .map_with_span(ExprKind::into_expr)
        .or(pipeline)
        .boxed();

        // indirections
        let term = term
            .then(
                ctrl('.')
                    .ignore_then(choice((
                        ident_part().map(IndirectionKind::Name),
                        ctrl('*').to(IndirectionKind::Star),
                        select! {
                            TokenKind::Literal(Literal::Integer(i)) => IndirectionKind::Position(i)
                        },
                    )))
                    .map_with_span(|f, s| (f, s))
                    .repeated(),
            )
            .foldl(|base, (field, span)| {
                let base = Box::new(base);
                ExprKind::Indirection { base, field }.into_expr(span)
            })
            .boxed();

        // Unary operators
        let term = term
            .clone()
            .or(operator_unary()
                .then(term.map(Box::new))
                .map(|(op, expr)| ExprKind::Unary(UnaryExpr { op, expr }))
                .map_with_span(ExprKind::into_expr))
            .boxed();

        // Ranges have five cases we need to parse:
        // x..y (bounded)
        // x..  (only start bound)
        // x    (no-op)
        //  ..y (only end bound)
        //  ..  (unbounded)
        #[derive(Clone)]
        enum RangeCase {
            NoOp(Expr),
            Range(Option<Expr>, Option<Expr>),
        }
        let term = choice((
            // with start bound (first 3 cases)
            term.clone()
                .then(choice((
                    // range and end bound
                    just(TokenKind::range(true, true))
                        .ignore_then(term.clone())
                        .map(|x| Some(Some(x))),
                    // range and no end bound
                    select! { TokenKind::Range { bind_left: true, .. } => Some(None) },
                    // no range
                    empty().to(None),
                )))
                .map(|(start, range)| {
                    if let Some(end) = range {
                        RangeCase::Range(Some(start), end)
                    } else {
                        RangeCase::NoOp(start)
                    }
                }),
            // only end bound
            select! { TokenKind::Range { bind_right: true, .. } => () }
                .ignore_then(term)
                .map(|range| RangeCase::Range(None, Some(range))),
            // unbounded
            select! { TokenKind::Range { .. } => RangeCase::Range(None, None) },
        ))
        .map_with_span(|case, span| match case {
            RangeCase::NoOp(x) => x,
            RangeCase::Range(start, end) => {
                let kind = ExprKind::Range(Range {
                    start: start.map(Box::new),
                    end: end.map(Box::new),
                });
                kind.into_expr(span)
            }
        })
        .boxed();

        // Binary operators
        let expr = term;
        // TODO: for `operator_pow` we need to do right-associative parsing
        // let expr = binary_op_parser_right(expr, operator_pow());
        let expr = binary_op_parser(expr, operator_mul());
        let expr = binary_op_parser(expr, operator_add());
        let expr = binary_op_parser(expr, operator_compare());
        let expr = binary_op_parser(expr, operator_coalesce());
        let expr = binary_op_parser(expr, operator_and());

        binary_op_parser(expr, operator_or())
    })
}

pub fn pipeline<E>(expr: E) -> impl Parser<TokenKind, Expr, Error = PError> + Clone
where
    E: Parser<TokenKind, Expr, Error = PError> + Clone,
{
    // expr has to be a param, because it can be either a normal expr() or a
    // recursive expr called from within expr(), which causes a stack overflow

    let pipe = ctrl('|').or(new_line().repeated().at_least(1).ignored());

    new_line()
        .repeated()
        .ignore_then(
            ident_part()
                .then_ignore(ctrl('='))
                .or_not()
                .then(expr)
                .map(|(alias, expr)| Expr { alias, ..expr })
                .separated_by(pipe)
                .at_least(1)
                .map_with_span(|exprs, span| {
                    // If there's only one expr, then we don't need to wrap it
                    // in a pipeline — just return the lone expr. Otherwise,
                    // wrap them in a pipeline.
                    exprs.into_iter().exactly_one().unwrap_or_else(|exprs| {
                        ExprKind::Pipeline(Pipeline {
                            exprs: exprs.collect(),
                        })
                        .into_expr(span)
                    })
                }),
        )
        .then_ignore(new_line().repeated())
        .labelled("pipeline")
}

pub fn binary_op_parser<'a, Term, Op>(
    term: Term,
    op: Op,
) -> impl Parser<TokenKind, Expr, Error = PError> + 'a + Clone
where
    Term: Parser<TokenKind, Expr, Error = PError> + 'a,
    Op: Parser<TokenKind, BinOp, Error = PError> + 'a,
{
    let term = term.map_with_span(|e, s| (e, s)).boxed();

    (term.clone())
        .then(op.then(term).repeated())
        .foldl(|left, (op, right)| {
            let span = Span {
                start: left.1.start,
                end: right.1.end,
                source_id: left.1.source_id,
            };
            let kind = ExprKind::Binary(BinaryExpr {
                left: Box::new(left.0),
                op,
                right: Box::new(right.0),
            });
            (ExprKind::into_expr(kind, span), span)
        })
        .map(|(e, _)| e)
        .boxed()
}

fn func_call<E>(expr: E) -> impl Parser<TokenKind, Expr, Error = PError> + Clone
where
    E: Parser<TokenKind, Expr, Error = PError> + Clone,
{
    let func_name = expr.clone();

    let named_arg = ident_part()
        .map(Some)
        .then_ignore(ctrl(':'))
        .then(expr.clone());

    let positional_arg =
        ident_part()
            .then_ignore(ctrl('='))
            .or_not()
            .then(expr)
            .map(|(alias, mut expr)| {
                expr.alias = alias.or(expr.alias);
                (None, expr)
            });

    func_name
        .then(named_arg.or(positional_arg).repeated())
        .validate(|(name, args), span, emit| {
            if args.is_empty() {
                return name.kind;
            }

            let mut named_args = HashMap::new();
            let mut positional = Vec::new();
            for (name, arg) in args {
                if let Some(name) = name {
                    if named_args.contains_key(&name) {
                        let err = PError::custom(span, "argument is used multiple times");
                        emit(err)
                    }
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
        .map_with_span(ExprKind::into_expr)
        .labelled("function call")
}

fn lambda_func<E>(expr: E) -> impl Parser<TokenKind, Expr, Error = PError> + Clone
where
    E: Parser<TokenKind, Expr, Error = PError> + Clone,
{
    let param = ident_part()
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then(ctrl(':').ignore_then(expr.clone().map(Box::new)).or_not());

    let generic_args = ident_part()
        .then_ignore(ctrl(':'))
        .then(type_expr().separated_by(ctrl('|')))
        .map(|(name, domain)| GenericTypeParam { name, domain })
        .separated_by(ctrl(','))
        .at_least(1)
        .delimited_by(ctrl('<'), ctrl('>'))
        .or_not()
        .map(|x| x.unwrap_or_default());

    choice((
        // func
        keyword("func").ignore_then(generic_args).then(
            param
                .clone()
                .separated_by(new_line().repeated())
                .allow_leading()
                .allow_trailing(),
        ),
        // plain
        param.repeated().map(|params| (Vec::new(), params)),
    ))
    .then_ignore(just(TokenKind::ArrowThin))
    // return type
    .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
    // body
    .then(func_call(expr))
    .map(|(((generic_type_params, params), return_ty), body)| {
        let (pos, name) = params
            .into_iter()
            .map(|((name, ty), default_value)| FuncParam {
                name,
                ty,
                default_value,
            })
            .partition(|p| p.default_value.is_none());

        Box::new(Func {
            params: pos,
            named_params: name,

            body: Box::new(body),
            return_ty,
            generic_type_params,
        })
    })
    .map(ExprKind::Func)
    .map_with_span(ExprKind::into_expr)
    .labelled("function definition")
}

pub fn ident() -> impl Parser<TokenKind, Ident, Error = PError> + Clone {
    ident_part()
        .separated_by(ctrl('.'))
        .at_least(1)
        .map(Ident::from_path::<String>)
}

fn operator_unary() -> impl Parser<TokenKind, UnOp, Error = PError> {
    (ctrl('+').to(UnOp::Add))
        .or(ctrl('-').to(UnOp::Neg))
        .or(ctrl('!').to(UnOp::Not))
        .or(just(TokenKind::Eq).to(UnOp::EqSelf))
}
// fn operator_pow() -> impl Parser<TokenKind, BinOp, Error = PError> {
//     just(TokenKind::Pow).to(BinOp::Pow)
// }
fn operator_mul() -> impl Parser<TokenKind, BinOp, Error = PError> {
    (just(TokenKind::DivInt).to(BinOp::DivInt))
        .or(ctrl('*').to(BinOp::Mul))
        .or(ctrl('/').to(BinOp::DivFloat))
        .or(ctrl('%').to(BinOp::Mod))
}
fn operator_add() -> impl Parser<TokenKind, BinOp, Error = PError> {
    (ctrl('+').to(BinOp::Add)).or(ctrl('-').to(BinOp::Sub))
}
fn operator_compare() -> impl Parser<TokenKind, BinOp, Error = PError> {
    choice((
        just(TokenKind::Eq).to(BinOp::Eq),
        just(TokenKind::Ne).to(BinOp::Ne),
        just(TokenKind::Lte).to(BinOp::Lte),
        just(TokenKind::Gte).to(BinOp::Gte),
        just(TokenKind::RegexSearch).to(BinOp::RegexSearch),
        ctrl('<').to(BinOp::Lt),
        ctrl('>').to(BinOp::Gt),
    ))
}
fn operator_and() -> impl Parser<TokenKind, BinOp, Error = PError> {
    just(TokenKind::And).to(BinOp::And)
}
pub fn operator_or() -> impl Parser<TokenKind, BinOp, Error = PError> {
    just(TokenKind::Or).to(BinOp::Or)
}
fn operator_coalesce() -> impl Parser<TokenKind, BinOp, Error = PError> {
    just(TokenKind::Coalesce).to(BinOp::Coalesce)
}
