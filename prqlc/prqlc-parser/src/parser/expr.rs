use std::collections::{hash_map::Entry, HashMap};

use chumsky;
use chumsky::input::BorrowInput;
use chumsky::prelude::*;
use itertools::Itertools;

use crate::lexer::lr;
use crate::lexer::lr::TokenKind;
use crate::parser::interpolation;
use crate::parser::pr::*;
use crate::parser::types::type_expr;
use crate::parser::{ctrl, ident_part, keyword, new_line, sequence, with_doc_comment};
use crate::span::Span;

use super::pipe;
use super::ParserError;

// Type aliases to reduce verbosity in binary operator parsers
type ExprWithSpan = (Expr, Span);
type BinOpChain = Vec<(BinOp, ExprWithSpan)>;

pub(crate) fn expr_call<'a, I>() -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    let expr = expr();

    choice((
        lambda_func(expr.clone()),
        func_call(expr.clone()),
        pipeline(expr),
    ))
}

pub(crate) fn expr<'a, I>() -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    recursive(|expr| {
        let literal = select_ref! { lr::Token { kind: TokenKind::Literal(lit), .. } => ExprKind::Literal(lit.clone()) };

        let ident_kind = ident().map(ExprKind::Ident);

        let internal = keyword("internal")
            .ignore_then(ident())
            .map(|x| x.to_string())
            .map(ExprKind::Internal);

        let nested_expr = with_doc_comment(
            lambda_func(expr.clone())
                .or(func_call(expr.clone()))
                .boxed(),
        );

        let tuple = tuple(nested_expr.clone());
        let array = array(nested_expr.clone());
        let pipeline_expr = {
            pipeline(nested_expr.clone())
                .padded_by(new_line().repeated())
                .delimited_by(ctrl('('), ctrl(')'))
        };
        let interpolation = interpolation();
        let case = case(expr.clone());

        let param = select_ref! { lr::Token { kind: TokenKind::Param(id), .. } => ExprKind::Param(id.clone()) };

        let term = with_doc_comment(
            choice((
                literal,
                internal,
                tuple,
                array,
                interpolation,
                ident_kind,
                case,
                param,
            ))
            .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span()))
            // No longer used given the TODO in `pipeline`; can remove if we
            // don't resolve.
            // .or(aliased(expr.clone()))
            .or(pipeline_expr),
        )
        .boxed();

        let term = unary(term);
        let term = range(term);

        // Binary operators
        let expr = term;
        let expr = binary_op_parser_right(expr, operator_pow());
        let expr = binary_op_parser(expr, operator_mul());
        let expr = binary_op_parser(expr, operator_add());
        let expr = binary_op_parser(expr, operator_compare());
        let expr = binary_op_parser(expr, operator_coalesce());
        let expr = binary_op_parser(expr, operator_and());

        binary_op_parser(expr, operator_or())
    })
}

fn tuple<'a, I>(
    nested_expr: impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
) -> impl Parser<'a, I, ExprKind, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    sequence(maybe_aliased(nested_expr))
        .delimited_by(ctrl('{'), ctrl('}'))
        .map(ExprKind::Tuple)
        .labelled("tuple")
}

fn array<'a, I>(
    expr: impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
) -> impl Parser<'a, I, ExprKind, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    sequence(expr)
        .delimited_by(ctrl('['), ctrl(']'))
        .map(ExprKind::Array)
        .labelled("array")
}

fn interpolation<'a, I>() -> impl Parser<'a, I, ExprKind, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! {
        lr::Token { kind: TokenKind::Interpolation('s', string), .. } => (ExprKind::SString as fn(_) -> _, string.clone()),
        lr::Token { kind: TokenKind::Interpolation('f', string), .. } => (ExprKind::FString as fn(_) -> _, string.clone()),
    }
    .validate(|(finish, string), extra, emit| {
        let span = extra.span();
        match interpolation::parse(string, span + 2) {
            Ok(items) => finish(items),
            Err(errors) => {
                for err in errors {
                    // Convert Error to Rich for emission
                    let err_span = err.span.unwrap_or(span);
                    // Use the reason's Display impl, not Error's Debug
                    let message = err.reason.to_string();
                    emit.emit(Rich::custom(err_span, message));
                }
                finish(vec![])
            }
        }
    })
    .labelled("interpolated string")
}

fn case<'a, I>(
    expr: impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
) -> impl Parser<'a, I, ExprKind, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    // The `nickname != null => nickname,` part
    let mapping = func_call(expr.clone())
        .map(Box::new)
        .then_ignore(select_ref! { lr::Token { kind: TokenKind::ArrowFat, .. } => () })
        .then(func_call(expr).map(Box::new))
        .map(|(condition, value)| SwitchCase { condition, value });

    keyword("case")
        .ignore_then(sequence(mapping).delimited_by(ctrl('['), ctrl(']')))
        .map(ExprKind::Case)
}

fn unary<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    expr.clone()
        .or(operator_unary()
            .then(expr.map(Box::new))
            .map(|(op, expr)| ExprKind::Unary(UnaryExpr { op, expr }))
            .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span())))
        .boxed()
}

fn range<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
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
    choice((
        // with start bound (first 3 cases)
        expr.clone()
            .then(choice((
                // range and end bound
                select_ref! { lr::Token { kind: TokenKind::Range { bind_left: true, bind_right: true }, .. } => () }
                    .ignore_then(expr.clone())
                    .map(|x| Some(Some(x))),
                // range and no end bound
                select_ref! { lr::Token { kind: TokenKind::Range { bind_left: true, .. }, .. } => Some(None) },
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
        select_ref! { lr::Token { kind: TokenKind::Range { bind_right: true, .. }, .. } => () }
            .ignore_then(expr)
            .map(|range| RangeCase::Range(None, Some(range))),
        // unbounded
        select_ref! { lr::Token { kind: TokenKind::Range { .. }, .. } => RangeCase::Range(None, None) },
    ))
    .map_with(|case, extra| {
        let span = extra.span();
        match case {
            RangeCase::NoOp(x) => x,
            RangeCase::Range(start, end) => {
                let kind = ExprKind::Range(Range {
                    start: start.map(Box::new),
                    end: end.map(Box::new),
                });
                kind.into_expr(span)
            }
        }
    })
}

/// A pipeline of `expr`, separated by pipes. Doesn't require parentheses.
pub(crate) fn pipeline<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    // expr has to be a param, because it can be either a normal expr() or a
    // recursive expr called from within expr(), which causes a stack overflow

    // TODO: do we need the `maybe_aliased` here rather than in `expr`? We had
    // tried `with_doc_comment(expr)` in #4775 (and push an aliased expr into
    // `expr`) but couldn't get it work.
    with_doc_comment(maybe_aliased(expr))
        .separated_by(pipe())
        .at_least(1)
        .collect::<Vec<_>>()
        .map_with(|exprs: Vec<Expr>, extra| {
            let span = extra.span();
            // If there's only one expr, then we don't need to wrap it
            // in a pipeline — just return the lone expr. Otherwise,
            // wrap them in a pipeline.
            exprs.into_iter().exactly_one().unwrap_or_else(|exprs| {
                ExprKind::Pipeline(Pipeline {
                    exprs: exprs.collect(),
                })
                .into_expr(span)
            })
        })
        .labelled("pipeline")
}

fn binary_op_parser<'a, I, Term, Op>(
    term: Term,
    op: Op,
) -> impl Parser<'a, I, Expr, ParserError<'a>> + 'a + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    Term: Parser<'a, I, Expr, ParserError<'a>> + 'a + Clone,
    Op: Parser<'a, I, BinOp, ParserError<'a>> + 'a + Clone,
{
    let term = term.map_with(|e, extra| (e, extra.span())).boxed();

    term.clone()
        .then(op.then(term).repeated().collect::<Vec<_>>())
        .map(|(first, rest): (ExprWithSpan, BinOpChain)| {
            rest.into_iter().fold(first, |left, (op, right)| {
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
        })
        .map(|(e, _)| e)
        .boxed()
}

pub(crate) fn binary_op_parser_right<'a, I, Term, Op>(
    term: Term,
    op: Op,
) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    Term: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
    Op: Parser<'a, I, BinOp, ParserError<'a>> + Clone + 'a,
{
    let term = term.map_with(|e, extra| (e, extra.span())).boxed();

    (term.clone())
        .then(op.then(term).repeated().collect::<Vec<_>>())
        .map(|(first, others): (ExprWithSpan, BinOpChain)| {
            // A transformation from this:
            // ```
            // first: e1
            // others: [(op1 e2) (op2 e3)]
            // ```
            // ... into:
            // ```
            // r: [(e1 op1) (e2 op2)]
            // e3
            // ```
            // .. so we can use foldr for right associativity.
            // We could use `(term.then(op)).repeated().then(term)` instead,
            // and have the correct structure from the get-go, but that would
            // perform miserably with simple expressions without operators, because
            // it would re-parse the term twice for each level of precedence we have.

            let mut free = first;
            let mut r = Vec::new();
            for (op, expr) in others {
                r.push((free, op));
                free = expr;
            }

            // Fold right manually
            r.into_iter().rfold(free, |right, (left, op)| {
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
                (kind.into_expr(span), span)
            })
        })
        .map(|(e, _)| e)
        .boxed()
}

// Can remove if we don't end up using this
#[allow(dead_code)]
#[cfg(not(coverage))]
fn aliased<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    let aliased = ident_part()
        .then_ignore(ctrl('='))
        .then(expr)
        .map(|(alias, mut expr)| {
            expr.alias = Some(alias);
            expr
        });
    // Because `expr` accounts for parentheses, and aliased is `x=$expr`, we
    // need to allow another layer of parentheses here.
    aliased
        .clone()
        .or(aliased.delimited_by(ctrl('('), ctrl(')')))
}

fn maybe_aliased<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    let aliased = ident_part()
        .then_ignore(ctrl('='))
        // This is added for `maybe_aliased`; possibly we should integrate
        // the funcs
        .or_not()
        .then(expr)
        .map(|(alias, mut expr)| {
            expr.alias = alias.or(expr.alias);
            expr
        });
    // Because `expr` accounts for parentheses, and aliased is `x=$expr`, we
    // need to allow another layer of parentheses here.
    aliased
        .clone()
        .or(aliased.delimited_by(ctrl('('), ctrl(')')))
}

fn func_call<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    let func_name = expr.clone();

    let named_arg = ident_part()
        .map(Some)
        .then_ignore(ctrl(':'))
        .then(expr.clone());

    // TODO: I think this possibly should be restructured. Currently in the case
    // of `derive x = 5`, the `x` is an alias of a single positional argument.
    // That then means we incorrectly allow something like `derive x = 5 y = 6`,
    // since there are two positional arguments each with an alias. This then
    // leads to quite confusing error messages.
    //
    // Instead, we could only allow a single alias per function call as the
    // first positional argument? (I worry that not simple though...).
    // Alternatively we could change the language to enforce tuples, so `derive
    // {x = 5}` were required. But we still need to account for the `join`
    // example below, which doesn't work so well in a tuple; so I'm not sure
    // this helps much.
    //
    // As a reminder, we need to account for `derive x = 5` and `join a=artists
    // (id==album_id)`.
    let positional_arg = maybe_aliased(expr.clone()).map(|e| (None, e));

    func_name
        .then(named_arg.or(positional_arg).repeated().collect::<Vec<_>>())
        .validate(
            |(name, args): (Expr, Vec<(Option<String>, Expr)>), extra, emit| {
                let span = extra.span();
                if args.is_empty() {
                    return name.kind;
                }

                let mut named_args = HashMap::new();
                let mut positional = Vec::new();

                for (name, arg) in args {
                    if let Some(name) = name {
                        match named_args.entry(name) {
                            Entry::Occupied(entry) => {
                                emit.emit(Rich::custom(
                                    span,
                                    format!("argument '{}' is used multiple times", entry.key()),
                                ));
                            }
                            Entry::Vacant(entry) => {
                                entry.insert(arg);
                            }
                        }
                    } else {
                        positional.push(arg);
                    }
                }

                ExprKind::FuncCall(FuncCall {
                    name: Box::new(name),
                    args: positional,
                    named_args,
                })
            },
        )
        .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span()))
        .labelled("function call")
}

fn lambda_func<'a, I, E>(expr: E) -> impl Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
    E: Parser<'a, I, Expr, ParserError<'a>> + Clone + 'a,
{
    let param = ident_part()
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then(ctrl(':').ignore_then(expr.clone().map(Box::new)).or_not());

    choice((
        // func
        keyword("func").ignore_then(
            param
                .clone()
                .separated_by(new_line().repeated())
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<_>>(),
        ),
        // plain
        param.repeated().collect(),
    ))
    .then_ignore(select_ref! { lr::Token { kind: TokenKind::ArrowThin, .. } => () })
    // return type
    .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
    // body
    .then(func_call(expr))
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
    .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span()))
    .labelled("function definition")
}

pub(crate) fn ident<'a, I>() -> impl Parser<'a, I, Ident, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    ident_part()
        .then_ignore(ctrl('.'))
        .repeated()
        .collect()
        .then(choice((ident_part(), ctrl('*').map(|_| "*".to_string()))))
        .map(|(mut parts, last): (Vec<String>, String)| {
            parts.push(last);
            Ident::from_path(parts)
        })
}

fn operator_unary<'a, I>() -> impl Parser<'a, I, UnOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    (ctrl('+').to(UnOp::Add))
        .or(ctrl('-').to(UnOp::Neg))
        .or(ctrl('!').to(UnOp::Not))
        .or(select_ref! { lr::Token { kind: TokenKind::Eq, .. } => UnOp::EqSelf })
}
fn operator_pow<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! { lr::Token { kind: TokenKind::Pow, .. } => BinOp::Pow }
}
fn operator_mul<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    (select_ref! { lr::Token { kind: TokenKind::DivInt, .. } => BinOp::DivInt })
        .or(ctrl('*').to(BinOp::Mul))
        .or(ctrl('/').to(BinOp::DivFloat))
        .or(ctrl('%').to(BinOp::Mod))
}
fn operator_add<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    (ctrl('+').to(BinOp::Add)).or(ctrl('-').to(BinOp::Sub))
}
fn operator_compare<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    choice((
        select_ref! { lr::Token { kind: TokenKind::Eq, .. } => BinOp::Eq },
        select_ref! { lr::Token { kind: TokenKind::Ne, .. } => BinOp::Ne },
        select_ref! { lr::Token { kind: TokenKind::Lte, .. } => BinOp::Lte },
        select_ref! { lr::Token { kind: TokenKind::Gte, .. } => BinOp::Gte },
        select_ref! { lr::Token { kind: TokenKind::RegexSearch, .. } => BinOp::RegexSearch },
        ctrl('<').to(BinOp::Lt),
        ctrl('>').to(BinOp::Gt),
    ))
}
fn operator_and<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! { lr::Token { kind: TokenKind::And, .. } => BinOp::And }
}
fn operator_or<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! { lr::Token { kind: TokenKind::Or, .. } => BinOp::Or }
}
fn operator_coalesce<'a, I>() -> impl Parser<'a, I, BinOp, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a>,
{
    select_ref! { lr::Token { kind: TokenKind::Coalesce, .. } => BinOp::Coalesce }
}

#[cfg(test)]
mod tests {

    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    use super::*;
    use crate::error::Error;
    use chumsky::span::SimpleSpan;

    // Macro to eliminate test helper boilerplate
    macro_rules! parse_test {
        ($source:expr, $parser:expr) => {{
            let tokens = crate::lexer::lex_source($source)?;
            let semantic_tokens: Vec<_> = tokens
                .0
                .into_iter()
                .filter(|t| !matches!(t.kind, TokenKind::Comment(_) | TokenKind::LineWrap(_)))
                .collect();
            let input = semantic_tokens
                .as_slice()
                .map_span(|simple_span: SimpleSpan| {
                    let start_idx = simple_span.start();
                    let end_idx = simple_span.end();

                    let start = semantic_tokens
                        .get(start_idx)
                        .map(|t| t.span.start)
                        .unwrap_or(0);
                    let end = semantic_tokens
                        .get(end_idx.saturating_sub(1))
                        .map(|t| t.span.end)
                        .unwrap_or(start);

                    Span {
                        start,
                        end,
                        source_id: 0,
                    }
                });
            let (ast, errors) = $parser.parse(input).into_output_errors();
            if !errors.is_empty() {
                return Err(errors.into_iter().map(Into::into).collect());
            }
            Ok(ast.unwrap())
        }};
    }

    fn parse_expr_call(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(expr_call())
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_tuple(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(tuple(expr()))
                .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span()))
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_any_expr(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(expr())
        )
    }

    fn parse_pipeline(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(pipeline(expr_call()))
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_case(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(case(expr()))
                .map_with(|kind, extra| ExprKind::into_expr(kind, extra.span()))
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_expr_call_complete(source: &str) -> Result<Expr, Vec<Error>> {
        parse_test!(
            source,
            new_line()
                .repeated()
                .collect::<Vec<_>>()
                .ignore_then(expr_call())
                .then_ignore(end())
        )
    }

    #[test]
    fn test_expr_call() {
        assert_yaml_snapshot!(
            parse_expr_call(r#"derive x = 5"#).unwrap(),
             @r#"
        FuncCall:
          name:
            Ident:
              - derive
            span: "0:0-6"
          args:
            - Literal:
                Integer: 5
              span: "0:11-12"
              alias: x
        span: "0:0-12"
        "#);

        assert_yaml_snapshot!(
            parse_expr_call(r#"aggregate {sum salary}"#).unwrap(),
             @r#"
        FuncCall:
          name:
            Ident:
              - aggregate
            span: "0:0-9"
          args:
            - Tuple:
                - FuncCall:
                    name:
                      Ident:
                        - sum
                      span: "0:11-14"
                    args:
                      - Ident:
                          - salary
                        span: "0:15-21"
                  span: "0:11-21"
              span: "0:10-22"
        span: "0:0-22"
        "#);
    }

    // The behavior that expr() doesn't parse aliases is tested by test_tuple

    #[test]
    fn test_tuple() {
        assert_yaml_snapshot!(
            parse_tuple(r#"{a = 5, b = 6}"#).unwrap(),
            @r#"
        Tuple:
          - Literal:
              Integer: 5
            span: "0:5-6"
            alias: a
          - Literal:
              Integer: 6
            span: "0:12-13"
            alias: b
        span: "0:0-14"
        "#);

        assert_debug_snapshot!(
            parse_tuple(r#"
            {a = 5
             b = 6}"#).unwrap_err(),
            @r#"
        [
            Error {
                kind: Error,
                span: Some(
                    0:33-34,
                ),
                reason: Expected {
                    who: None,
                    expected: "new line or something else",
                    found: "b",
                },
                hints: [],
                code: None,
            },
        ]
        "#);

        assert_yaml_snapshot!(parse_tuple(r#"{d_str = (d | date.to_text "%Y/%m/%d")}"#).unwrap(),
        @r#"
        Tuple:
          - Pipeline:
              exprs:
                - Ident:
                    - d
                  span: "0:10-11"
                - FuncCall:
                    name:
                      Ident:
                        - date
                        - to_text
                      span: "0:14-26"
                    args:
                      - Literal:
                          String: "%Y/%m/%d"
                        span: "0:27-37"
                  span: "0:14-37"
            span: "0:10-37"
            alias: d_str
        span: "0:0-39"
        "#);
    }

    #[test]
    fn test_expr() {
        assert_yaml_snapshot!(
            parse_any_expr(r#"5+5"#).unwrap(),
             @r#"
        Binary:
          left:
            Literal:
              Integer: 5
            span: "0:0-1"
          op: Add
          right:
            Literal:
              Integer: 5
            span: "0:2-3"
        span: "0:0-3"
        "#);
    }

    #[test]
    fn test_pipeline() {
        assert_yaml_snapshot!(
            parse_pipeline(r#"
            (
              from artists
              derive x = 5
            )
            "#).unwrap(),
            @r#"
        Pipeline:
          exprs:
            - FuncCall:
                name:
                  Ident:
                    - from
                  span: "0:29-33"
                args:
                  - Ident:
                      - artists
                    span: "0:34-41"
              span: "0:29-41"
            - FuncCall:
                name:
                  Ident:
                    - derive
                  span: "0:56-62"
                args:
                  - Literal:
                      Integer: 5
                    span: "0:67-68"
                    alias: x
              span: "0:56-68"
        span: "0:13-82"
        "#);
    }

    #[test]
    fn test_case() {
        assert_yaml_snapshot!(
            parse_case(r#"

        case [

            nickname != null => nickname,
            true => null

        ]
            "#).unwrap(),
        @r#"
        Case:
          - condition:
              Binary:
                left:
                  Ident:
                    - nickname
                  span: "0:30-38"
                op: Ne
                right:
                  Literal: "Null"
                  span: "0:42-46"
              span: "0:30-46"
            value:
              Ident:
                - nickname
              span: "0:50-58"
          - condition:
              Literal:
                Boolean: true
              span: "0:72-76"
            value:
              Literal: "Null"
              span: "0:80-84"
        span: "0:0-95"
        "#);
    }

    // this should return an error but doesn't yet
    #[should_panic]
    #[test]
    fn should_error_01() {
        assert_debug_snapshot!(
            parse_expr_call_complete(r#"derive {x = y z = 3}"#).unwrap_err(),
            @r###"
        "###);
    }

    #[test]
    fn tuple_missing_comma() {
        assert_debug_snapshot!(
            parse_expr_call_complete(r#"{
              x = y
              z = 3
            }"#).unwrap_err(),
            @r#"
        [
            Error {
                kind: Error,
                span: Some(
                    0:36-37,
                ),
                reason: Expected {
                    who: None,
                    expected: "new line or something else",
                    found: "z",
                },
                hints: [],
                code: None,
            },
        ]
        "#);
    }

    #[test]
    fn args_in_parens() {
        // Ensure function arguments allow parentheses
        assert_yaml_snapshot!(
            parse_expr_call_complete(r#"f (a) b"#).unwrap(), @r#"
        FuncCall:
          name:
            Ident:
              - f
            span: "0:0-1"
          args:
            - Ident:
                - a
              span: "0:3-4"
            - Ident:
                - b
              span: "0:6-7"
        span: "0:0-7"
        "#);

        assert_yaml_snapshot!(
            parse_expr_call_complete(r#"f (a=2) b"#).unwrap(), @r#"
        FuncCall:
          name:
            Ident:
              - f
            span: "0:0-1"
          args:
            - Literal:
                Integer: 2
              span: "0:5-6"
              alias: a
            - Ident:
                - b
              span: "0:8-9"
        span: "0:0-9"
        "#);

        assert_yaml_snapshot!(
            parse_expr_call_complete(r#"f (a b)"#).unwrap(), @r#"
        FuncCall:
          name:
            Ident:
              - f
            span: "0:0-1"
          args:
            - FuncCall:
                name:
                  Ident:
                    - a
                  span: "0:3-4"
                args:
                  - Ident:
                      - b
                    span: "0:5-6"
              span: "0:3-6"
        span: "0:0-7"
        "#);
    }

    #[test]
    fn pipeline_starting_with_alias_expr() {
        let source = r#"
    (
      tbl
      select t.date
    )
    "#;

        assert_yaml_snapshot!(parse_pipeline(source).unwrap(), @r#"
        Pipeline:
          exprs:
            - Ident:
                - tbl
              span: "0:13-16"
            - FuncCall:
                name:
                  Ident:
                    - select
                  span: "0:23-29"
                args:
                  - Ident:
                      - t
                      - date
                    span: "0:30-36"
              span: "0:23-36"
        span: "0:5-42"
        "#);

        let source = r#"
    (
      t = tbl
      select t.date
    )
    "#;

        assert_yaml_snapshot!(parse_pipeline(source).unwrap(), @r#"
        Pipeline:
          exprs:
            - Ident:
                - tbl
              span: "0:17-20"
              alias: t
            - FuncCall:
                name:
                  Ident:
                    - select
                  span: "0:27-33"
                args:
                  - Ident:
                      - t
                      - date
                    span: "0:34-40"
              span: "0:27-40"
        span: "0:5-46"
        "#);
    }
}
