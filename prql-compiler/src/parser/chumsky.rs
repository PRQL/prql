#![allow(dead_code)]

use std::collections::HashMap;

use chumsky::prelude::*;
use semver::VersionReq;

use crate::{ast::pl::*, Span};

pub fn source() -> impl Parser<char, Vec<Stmt>, Error = Simple<char>> {
    let stmt = query_def().or(main_pipeline());

    stmt.separated_by(just('\n'))
        .padded_by(whitespace().or(just('\n').to(())).repeated())
        .then_ignore(end())
}

fn main_pipeline() -> impl Parser<char, Stmt, Error = Simple<char>> {
    expr_call()
        .separated_by(pipe())
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

fn query_def() -> impl Parser<char, Stmt, Error = Simple<char>> {
    str("prql")
        .ignore_then(
            // named arg
            ident_part()
                .then_ignore(just(':').padded_by(whitespace().or_not()))
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

pub fn expr_call() -> impl Parser<char, Expr, Error = Simple<char>> {
    recursive(|expr| {
        let literal = literal().map(ExprKind::Literal);

        let ident_kind = ident().map(ExprKind::Ident);

        let list = just('[')
            .ignore_then(
                ident_part()
                    .then_ignore(just('=').padded_by(whitespace().or_not()))
                    .or_not()
                    .then(expr.clone())
                    .map(|(alias, expr)| Expr { alias, ..expr })
                    .padded_by(whitespace().or(just('\n').to(())).repeated())
                    .separated_by(just(','))
                    .allow_trailing(),
            )
            .then_ignore(just(']'))
            .map(ExprKind::List)
            .labelled("list");

        let pipeline = just('(')
            .ignore_then(
                expr.clone()
                    .padded_by(whitespace().or_not())
                    .separated_by(pipe()),
            )
            .then_ignore(just(')'))
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

fn binary_op_parser<'a, O>(
    term: BoxedParser<'a, char, Expr, Simple<char>>,
    op: O,
) -> BoxedParser<char, Expr, Simple<char>>
where
    O: Parser<char, BinOp, Error = Simple<char>> + 'a,
{
    let term = term.map_with_span(|e, s| (e, s));

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

fn func_call(
    expr: Recursive<char, Expr, Simple<char>>,
) -> impl Parser<char, ExprKind, Error = Simple<char>> + '_ {
    let name = ident()
        .boxed()
        .map(ExprKind::Ident)
        .map_with_span(into_expr);

    let named_arg = ident_part()
        .map(Some)
        .then_ignore(just(':').padded_by(whitespace().or_not()))
        .then(expr.clone());

    let assign_call = ident_part()
        .then_ignore(just('=').padded_by(whitespace().or_not()))
        .then(expr.clone())
        .map(|(alias, expr)| Expr {
            alias: Some(alias),
            ..expr
        });
    let positional_arg = assign_call.or(expr.clone()).map(|expr| (None, expr));

    let args = whitespace()
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

fn literal() -> impl Parser<char, Literal, Error = Simple<char>> {
    let exp = just('e').or(just('E')).ignore_then(
        just('+')
            .or(just('-'))
            .or_not()
            .chain::<char, _, _>(text::digits(10)),
    );

    let number_part = filter(|c: &char| c.is_digit(10) && *c != '0')
        .map(Some)
        .chain::<_, Vec<_>, _>(filter(move |c: &char| c.is_digit(10) || *c == '_').repeated())
        .collect()
        .or(just('0').map(|c| vec![c]));

    let frac = just('.').chain(number_part);

    let number = just('+')
        .or(just('-'))
        .or_not()
        .chain::<char, _, _>(number_part)
        .chain::<char, _, _>(frac.or_not().flatten())
        .chain::<char, _, _>(exp.or_not().flatten())
        .try_map(|chars, span| {
            // pest is responsible for ensuring these are in the correct place,
            // so we just need to remove them.
            let str = chars.into_iter().filter(|c| *c != '_').collect::<String>();

            if let Ok(i) = str.parse::<i64>() {
                Ok(Literal::Integer(i))
            } else if let Ok(f) = str.parse::<f64>() {
                Ok(Literal::Float(f))
            } else {
                Err(Simple::custom(span, "invalid number"))
            }
        })
        .labelled("number");

    let string = string();

    let bool = (str("true").to(true))
        .or(str("false").to(false))
        .map(Literal::Boolean);

    let null = str("null").to(Literal::Null);

    let value_and_unit = number_part
        .then(
            str("microseconds")
                .or(str("milliseconds"))
                .or(str("seconds"))
                .or(str("minutes"))
                .or(str("hours"))
                .or(str("days"))
                .or(str("weeks"))
                .or(str("months"))
                .or(str("years")),
        )
        .try_map(|(number, unit), span| {
            let str = number.into_iter().filter(|c| *c != '_').collect::<String>();
            if let Ok(n) = str.parse::<i64>() {
                let unit = unit.to_string();
                Ok(ValueAndUnit { n, unit })
            } else {
                Err(Simple::custom(span, "invalid number"))
            }
        })
        .map(Literal::ValueAndUnit);

    // TODO: timestamp
    // TODO: date
    // TODO: time
    // TODO: "(" ~ literal ~ ")" }  --- should this even be here?

    string.or(number).or(bool).or(null).or(value_and_unit)
}

fn string() -> impl Parser<char, Literal, Error = Simple<char>> {
    let escape = just('\\').ignore_then(
        just('\\')
            .or(just('/'))
            .or(just('"'))
            .or(just('b').to('\x08'))
            .or(just('f').to('\x0C'))
            .or(just('n').to('\n'))
            .or(just('r').to('\r'))
            .or(just('t').to('\t'))
            .or(just('u').ignore_then(
                filter(|c: &char| c.is_digit(16))
                    .repeated()
                    .exactly(4)
                    .collect::<String>()
                    .validate(|digits, span, emit| {
                        char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap_or_else(
                            || {
                                emit(Simple::custom(span, "invalid unicode character"));
                                '\u{FFFD}' // unicode replacement character
                            },
                        )
                    }),
            )),
    );

    // TODO: multi-quoted strings (this is just parsing JSON strings)
    just('"')
        .ignore_then(filter(|c| *c != '\\' && *c != '"').or(escape).repeated())
        .then_ignore(just('"'))
        .collect::<String>()
        .map(Literal::String)
        .labelled("string")
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
    (just("and").to(BinOp::And))
        .or(just("or").to(BinOp::Or))
        .then_ignore(whitespace())
}
fn operator_coalesce() -> impl Parser<char, BinOp, Error = Simple<char>> {
    just("??").map(|_| BinOp::Coalesce)
}

fn pipe() -> impl Parser<char, char, Error = Simple<char>> {
    just('|')
        .or(just('\n')
            .repeated()
            .at_least(1)
            .map(|mut chars| chars.pop().unwrap()))
        .padded_by(whitespace().or_not())
}

fn whitespace() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    filter(|c: &char| *c == '\t' || *c == ' ')
        .repeated()
        .at_least(1)
        .to(())
}

fn str(chars: &str) -> impl Parser<char, &str, Error = Simple<char>> + '_ {
    just(chars)
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
