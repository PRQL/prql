use std::collections::HashMap;

use chumsky::prelude::*;

use crate::ast::pl::*;

use super::common::*;
use super::interpolation;
use super::lexer::Token;

pub fn expr_call() -> impl Parser<Token, Expr, Error = Simple<Token>> {
    func_call(expr())
}

pub fn expr() -> impl Parser<Token, Expr, Error = Simple<Token>> + Clone {
    recursive(|expr| {
        let literal = select! { Token::Literal(lit) => ExprKind::Literal(lit) };

        let ident_kind = ident().map(ExprKind::Ident);

        let nested_expr = pipeline(func_call(expr.clone())).boxed();

        let list = ident_part()
            .then_ignore(ctrl('='))
            .or_not()
            .then(nested_expr.clone().map_with_span(into_expr))
            .map(|(alias, expr)| Expr { alias, ..expr })
            .padded_by(new_line().repeated())
            .separated_by(ctrl(','))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('['), ctrl(']'))
            .recover_with(nested_delimiters(
                Token::Control('['),
                Token::Control(']'),
                [
                    (Token::Control('['), Token::Control(']')),
                    (Token::Control('('), Token::Control(')')),
                ],
                |_| vec![],
            ))
            .map(ExprKind::List)
            .labelled("list");

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
                    |_| Expr::null().kind,
                ));

        let interpolation =
            select! {
                Token::Interpolation('s', string) => (ExprKind::SString as fn(_) -> _, string),
                Token::Interpolation('f', string) => (ExprKind::FString as fn(_) -> _, string),
            }
            .validate(|(finish, string), span: std::ops::Range<usize>, emit| {
                match interpolation::parse(string, span.start + 2) {
                    Ok(items) => finish(items),
                    Err(errors) => {
                        for err in errors {
                            emit(err)
                        }
                        finish(vec![])
                    }
                }
            });

        let case = keyword("case")
            .ignore_then(
                func_call(expr.clone())
                    .then_ignore(just(Token::ArrowDouble))
                    .then(func_call(expr))
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
            list,
            pipeline,
            interpolation,
            ident_kind,
            case,
            param,
        ))
        .map_with_span(into_expr)
        .boxed();

        // Unary operators
        let term = term
            .clone()
            .or(operator_unary()
                .then(term.map(Box::new))
                .map(|(op, expr)| ExprKind::Unary { op, expr })
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

pub fn pipeline<E>(expr: E) -> impl Parser<Token, ExprKind, Error = Simple<Token>>
where
    E: Parser<Token, Expr, Error = Simple<Token>>,
{
    // expr has to be a param, because it can be either a normal expr() or
    // a recursive expr called from within expr()

    new_line()
        .repeated()
        .ignore_then(
            expr.separated_by(ctrl('|').or(new_line().repeated().at_least(1).ignored()))
                .at_least(1)
                .map(|mut exprs| {
                    if exprs.len() == 1 {
                        exprs.remove(0).kind
                    } else {
                        ExprKind::Pipeline(Pipeline { exprs })
                    }
                }),
        )
        .then_ignore(new_line().repeated())
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
        .then(op.then(term).repeated())
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
        .then_ignore(ctrl(':'))
        .then(expr.clone());

    let assign_call =
        ident_part()
            .then_ignore(ctrl('='))
            .then(expr.clone())
            .map(|(alias, expr)| Expr {
                alias: Some(alias),
                ..expr
            });
    let positional_arg = assign_call.or(expr).map(|expr| (None, expr));

    let args = named_arg.or(positional_arg).repeated();

    func.then(args)
        .validate(|(name, args), span, emit| {
            if args.is_empty() {
                return name.kind;
            }

            let mut named_args = HashMap::new();
            let mut positional = Vec::new();
            for (name, arg) in args {
                if let Some(name) = name {
                    if named_args.contains_key(&name) {
                        let err = Simple::custom(span.clone(), "argument is used multiple times");
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

pub fn ident() -> impl Parser<Token, Ident, Error = Simple<Token>> {
    let star = ctrl('*').to("*".to_string());

    ident_part()
        .chain(ctrl('.').ignore_then(ident_part().or(star)).repeated())
        .map(Ident::from_path::<String>)
        .labelled("identifier")
}

fn operator_unary() -> impl Parser<Token, UnOp, Error = Simple<Token>> {
    (ctrl('+').to(UnOp::Add))
        .or(ctrl('-').to(UnOp::Neg))
        .or(ctrl('!').to(UnOp::Not))
        .or(just(Token::Eq).to(UnOp::EqSelf))
}
fn operator_mul() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl('*').to(BinOp::Mul))
        .or(ctrl('/').to(BinOp::Div))
        .or(ctrl('%').to(BinOp::Mod))
}
fn operator_add() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (ctrl('+').to(BinOp::Add)).or(ctrl('-').to(BinOp::Sub))
}
fn operator_compare() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    (just(Token::Eq).to(BinOp::Eq))
        .or(just(Token::Ne).to(BinOp::Ne))
        .or(just(Token::Lte).to(BinOp::Lte))
        .or(just(Token::Gte).to(BinOp::Gte))
        .or(ctrl('<').to(BinOp::Lt))
        .or(ctrl('>').to(BinOp::Gt))
}
fn operator_and() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    just(Token::And).to(BinOp::And)
}
fn operator_or() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    just(Token::Or).to(BinOp::Or)
}
fn operator_coalesce() -> impl Parser<Token, BinOp, Error = Simple<Token>> {
    just(Token::Coalesce).to(BinOp::Coalesce)
}
