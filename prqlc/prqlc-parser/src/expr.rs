use std::collections::HashMap;

use chumsky::prelude::*;

use prqlc_ast::expr::*;
use prqlc_ast::Span;

use crate::types::type_expr;

use super::common::*;
use super::interpolation;
use super::lexer::Token;
use super::span::ParserSpan;

pub fn expr_call() -> impl Parser<Token, Expr, Error = PError> {
    let expr = expr();

    lambda_func(expr.clone()).or(func_call(expr))
}

pub fn expr() -> impl Parser<Token, Expr, Error = PError> + Clone {
    recursive(|expr| {
        let literal = select! { Token::Literal(lit) => ExprKind::Literal(lit) };

        let ident_kind = ident().map(ExprKind::Ident);

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
                Token::Control('{'),
                Token::Control('}'),
                [
                    (Token::Control('{'), Token::Control('}')),
                    (Token::Control('('), Token::Control(')')),
                    (Token::Control('['), Token::Control(']')),
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
                Token::Control('['),
                Token::Control(']'),
                [
                    (Token::Control('{'), Token::Control('}')),
                    (Token::Control('('), Token::Control(')')),
                    (Token::Control('['), Token::Control(']')),
                ],
                |_| vec![],
            ))
            .map(ExprKind::Array)
            .labelled("array");

        let pipeline =
            nested_expr
                .delimited_by(ctrl('('), ctrl(')'))
                .recover_with(nested_delimiters(
                    Token::Control('('),
                    Token::Control(')'),
                    [
                        (Token::Control('['), Token::Control(']')),
                        (Token::Control('('), Token::Control(')')),
                    ],
                    |_| Expr::new(ExprKind::Literal(Literal::Null)),
                ));

        let interpolation = select! {
            Token::Interpolation('s', string) => (ExprKind::SString as fn(_) -> _, string),
            Token::Interpolation('f', string) => (ExprKind::FString as fn(_) -> _, string),
        }
        .validate(|(finish, string), span: ParserSpan, emit| {
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
                    .then_ignore(just(Token::ArrowFat))
                    .then(func_call(expr.clone()).map(Box::new))
                    .map(|(condition, value)| SwitchCase { condition, value })
                    .padded_by(new_line().repeated())
                    .separated_by(ctrl(','))
                    .allow_trailing()
                    .then_ignore(new_line().repeated())
                    .delimited_by(ctrl('['), ctrl(']')),
            )
            .map(ExprKind::Case);

        let param = select! { Token::Param(id) => ExprKind::Param(id) };

        let term = choice((
            literal,
            tuple,
            array,
            interpolation,
            ident_kind,
            case,
            param,
        ))
        .map_with_span(into_expr)
        .or(pipeline)
        .boxed();

        // Unary operators
        let term = term
            .clone()
            .or(operator_unary()
                .then(term.map(Box::new))
                .map(|(op, expr)| ExprKind::Unary(UnaryExpr { op, expr }))
                .map_with_span(into_expr))
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
                    just(Token::range(true, true))
                        .ignore_then(term.clone())
                        .map(|x| Some(Some(x))),
                    // range and no end bound
                    select! { Token::Range { bind_left: true, .. } => Some(None) },
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
            select! { Token::Range { bind_right: true, .. } => () }
                .ignore_then(term)
                .map(|range| RangeCase::Range(None, Some(range))),
            // unbounded
            select! { Token::Range { .. } => RangeCase::Range(None, None) },
        ))
        .map_with_span(|case, span| match case {
            RangeCase::NoOp(x) => x,
            RangeCase::Range(start, end) => {
                let kind = ExprKind::Range(Range {
                    start: start.map(Box::new),
                    end: end.map(Box::new),
                });
                into_expr(kind, span)
            }
        })
        .boxed();

        // Binary operators
        let expr = term;
        let expr = binary_op_parser(expr, operator_mul());
        let expr = binary_op_parser(expr, operator_add());
        let expr = binary_op_parser(expr, operator_compare());
        let expr = binary_op_parser(expr, operator_coalesce());
        let expr = binary_op_parser(expr, operator_and());

        binary_op_parser(expr, operator_or())
    })
}

pub fn pipeline<E>(expr: E) -> impl Parser<Token, Expr, Error = PError>
where
    E: Parser<Token, Expr, Error = PError>,
{
    // expr has to be a param, because it can be either a normal expr() or
    // a recursive expr called from within expr()

    new_line()
        .repeated()
        .ignore_then(
            ident_part()
                .then_ignore(ctrl('='))
                .or_not()
                .then(expr)
                .map(|(alias, mut expr)| {
                    expr.alias = alias.or(expr.alias);
                    expr
                })
                .separated_by(ctrl('|').or(new_line().repeated().at_least(1).ignored()))
                .at_least(1)
                .map_with_span(|mut exprs, span| {
                    if exprs.len() == 1 {
                        exprs.remove(0)
                    } else {
                        into_expr(ExprKind::Pipeline(Pipeline { exprs }), span)
                    }
                }),
        )
        .then_ignore(new_line().repeated())
        .labelled("pipeline")
}

pub fn binary_op_parser<'a, Term, Op>(
    term: Term,
    op: Op,
) -> impl Parser<Token, Expr, Error = PError> + 'a
where
    Term: Parser<Token, Expr, Error = PError> + 'a,
    Op: Parser<Token, BinOp, Error = PError> + 'a,
{
    let term = term.map_with_span(|e, s| (e, s)).boxed();

    (term.clone())
        .then(op.then(term).repeated())
        .foldl(|left, (op, right)| {
            let span = ParserSpan(Span {
                start: left.1.start,
                end: right.1.end,
                source_id: left.1.source_id,
            });
            let kind = ExprKind::Binary(BinaryExpr {
                left: Box::new(left.0),
                op,
                right: Box::new(right.0),
            });
            (into_expr(kind, span), span)
        })
        .map(|(e, _)| e)
        .boxed()
}

fn func_call<E>(expr: E) -> impl Parser<Token, Expr, Error = PError>
where
    E: Parser<Token, Expr, Error = PError> + Clone,
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
                        let err = Simple::custom(span, "argument is used multiple times");
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
        .map_with_span(into_expr)
        .labelled("function call")
}

fn lambda_func<E>(expr: E) -> impl Parser<Token, Expr, Error = PError>
where
    E: Parser<Token, Expr, Error = PError> + Clone + 'static,
{
    let param = ident_part()
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then(ctrl(':').ignore_then(expr.clone().map(Box::new)).or_not())
        .boxed();

    let internal = keyword("internal")
        .ignore_then(ident())
        .map(|x| x.to_string())
        .map(ExprKind::Internal)
        .map_with_span(into_expr);

    choice((
        // func
        keyword("func").ignore_then(
            param
                .clone()
                .separated_by(new_line().repeated())
                .allow_leading()
                .allow_trailing(),
        ),
        // plain
        param.repeated(),
    ))
    .then_ignore(just(Token::ArrowThin))
    // return type
    .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
    // body
    .then(choice((internal, func_call(expr))))
    .map(|((params, return_ty), body)| {
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
        })
    })
    .map(ExprKind::Func)
    .map_with_span(into_expr)
    .labelled("function definition")
}

pub fn ident() -> impl Parser<Token, Ident, Error = PError> {
    let star = ctrl('*').to("*".to_string());

    ident_part()
        .chain(ctrl('.').ignore_then(ident_part().or(star)).repeated())
        .map(Ident::from_path::<String>)
}

fn operator_unary() -> impl Parser<Token, UnOp, Error = PError> {
    (ctrl('+').to(UnOp::Add))
        .or(ctrl('-').to(UnOp::Neg))
        .or(ctrl('!').to(UnOp::Not))
        .or(just(Token::Eq).to(UnOp::EqSelf))
}
fn operator_mul() -> impl Parser<Token, BinOp, Error = PError> {
    (just(Token::DivInt).to(BinOp::DivInt))
        .or(just(Token::Pow).to(BinOp::Pow))
        .or(ctrl('*').to(BinOp::Mul))
        .or(ctrl('/').to(BinOp::DivFloat))
        .or(ctrl('%').to(BinOp::Mod))
}
fn operator_add() -> impl Parser<Token, BinOp, Error = PError> {
    (ctrl('+').to(BinOp::Add)).or(ctrl('-').to(BinOp::Sub))
}
fn operator_compare() -> impl Parser<Token, BinOp, Error = PError> {
    choice((
        just(Token::Eq).to(BinOp::Eq),
        just(Token::Ne).to(BinOp::Ne),
        just(Token::Lte).to(BinOp::Lte),
        just(Token::Gte).to(BinOp::Gte),
        just(Token::RegexSearch).to(BinOp::RegexSearch),
        ctrl('<').to(BinOp::Lt),
        ctrl('>').to(BinOp::Gt),
    ))
}
fn operator_and() -> impl Parser<Token, BinOp, Error = PError> {
    just(Token::And).to(BinOp::And)
}
pub fn operator_or() -> impl Parser<Token, BinOp, Error = PError> {
    just(Token::Or).to(BinOp::Or)
}
fn operator_coalesce() -> impl Parser<Token, BinOp, Error = PError> {
    just(Token::Coalesce).to(BinOp::Coalesce)
}
