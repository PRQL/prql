//! This module contains the parser, which is responsible for converting a tree
//! of pest pairs into a tree of AST Items. It has a small function to call into
//! pest to get the parse tree / concrete syntax tree, and then a large
//! function for turning that into PRQL AST.
mod expr;
mod lexer;
mod stmt;

use std::collections::HashMap;
use std::str::FromStr;

use ::chumsky::error::SimpleReason;
use ::chumsky::Stream;
use anyhow::bail;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;

use self::lexer::Token;

use super::ast::pl::*;
use super::utils::*;
use crate::error::{Error, Errors, Reason, Span};

mod common {
    use chumsky::prelude::*;

    use super::lexer::Token;
    use crate::{ast::pl::*, Span};

    pub fn ident_part() -> impl Parser<Token, String, Error = Simple<Token>> {
        select! { Token::Ident(ident) => ident }
    }

    pub fn keyword(kw: &str) -> impl Parser<Token, (), Error = Simple<Token>> + '_ {
        select! { Token::Ident(ident) if ident == kw => () }
    }

    pub fn whitespace() -> impl Parser<Token, (), Error = Simple<Token>> + Clone {
        just(Token::Whitespace).repeated().at_least(1).ignored()
    }

    pub fn new_line() -> impl Parser<Token, (), Error = Simple<Token>> + Clone {
        just(Token::NewLine).ignored()
    }

    pub fn ctrl(chars: &'static str) -> impl Parser<Token, (), Error = Simple<Token>> {
        select! { Token::Control(str) if str == chars => () }
    }

    pub fn into_stmt(kind: StmtKind, span: std::ops::Range<usize>) -> Stmt {
        Stmt {
            span: into_span(span),
            ..Stmt::from(kind)
        }
    }

    pub fn into_expr(kind: ExprKind, span: std::ops::Range<usize>) -> Expr {
        Expr {
            span: into_span(span),
            ..Expr::from(kind)
        }
    }

    pub fn into_span(span: std::ops::Range<usize>) -> Option<Span> {
        Some(Span {
            start: span.start,
            end: span.end,
        })
    }
}

#[derive(Parser)]
#[grammar = "parser/prql.pest"]
struct PrqlParser;

pub(crate) type PestError = pest::error::Error<Rule>;
pub(crate) type PestRule = Rule;

/// Build PL AST from a PRQL query string.
pub fn parse(string: &str) -> Result<Vec<Stmt>> {
    let mut errors = Vec::new();

    let (tokens, lex_errors) = ::chumsky::Parser::parse_recovery_verbose(&lexer::lexer(), string);
    dbg!(&tokens);

    errors.extend(lex_errors.into_iter().map(convert_char_error));

    let ast = if let Some(tokens) = tokens {
        let len = string.chars().count();
        let stream = Stream::from_iter(len..len + 1, tokens.into_iter());

        let (ast, parse_errors) =
            ::chumsky::Parser::parse_recovery_verbose(&stmt::source(), stream);

        errors.extend(parse_errors.into_iter().map(convert_token_error));

        ast
    } else {
        None
    };

    if errors.is_empty() {
        Ok(ast.unwrap_or_default())
    } else {
        Err(Errors(errors).into())
    }
}

fn convert_char_error(e: ::chumsky::prelude::Simple<char>) -> Error {
    dbg!(&e);

    let span = common::into_span(e.span());

    if let SimpleReason::Custom(message) = e.reason() {
        return Error::new_simple(message).with_span(span);
    }

    let expected = e
        .expected()
        .filter_map(|t| t.as_ref().map(|c| format!("{c:?}")))
        .collect_vec();

    let found = e.found().map(|c| c.to_string()).unwrap_or_default();

    if expected.is_empty() {
        Error::new(Reason::Unexpected { found })
    } else {
        let expected = expected.join(", ");

        Error::new(Reason::Expected {
            who: None,
            expected,
            found,
        })
    }
    .with_span(span)
}

fn convert_token_error(e: ::chumsky::prelude::Simple<Token>) -> Error {
    dbg!(&e);

    let span = common::into_span(e.span());

    if let SimpleReason::Custom(message) = e.reason() {
        return Error::new_simple(message).with_span(span);
    }

    let just_whitespace = e
        .expected()
        .all(|t| matches!(t, None | Some(Token::Whitespace | Token::NewLine)));
    let expected = e
        .expected()
        .filter(|t| {
            if just_whitespace {
                true
            } else {
                !matches!(t, Some(Token::Whitespace | Token::NewLine))
            }
        })
        .filter_map(|t| t.as_ref().map(|e| e.to_string()))
        .collect_vec();

    let found = e.found().map(|c| c.to_string()).unwrap_or_default();

    if expected.is_empty() {
        Error::new(Reason::Unexpected { found })
    } else {
        let expected = expected.join(", ");

        Error::new(Reason::Expected {
            who: None,
            expected,
            found,
        })
    }
    .with_span(span)
}

/// Parse a string into a parse tree / concrete syntax tree, made up of pest Pairs.
#[allow(dead_code)]
fn parse_tree_of_str(source: &str, rule: Rule) -> Result<Pairs<Rule>> {
    Ok(PrqlParser::parse(rule, source)?)
}

/// Parses a parse tree of pest Pairs into a list of statements.
#[allow(dead_code)]
fn stmts_of_parse_pairs(pairs: Pairs<Rule>) -> Result<Vec<Stmt>> {
    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(stmt_of_parse_pair)
        .collect()
}

#[allow(dead_code)]
fn stmt_of_parse_pair(pair: Pair<Rule>) -> Result<Stmt> {
    let span = pair.as_span();
    let rule = pair.as_rule();

    let kind = match rule {
        Rule::pipeline_stmt => {
            let pipeline = expr_of_parse_pair(pair.into_inner().next().unwrap())?;
            StmtKind::Main(Box::new(pipeline))
        }
        Rule::query_def => {
            let mut params: HashMap<_, _> = pair
                .into_inner()
                .map(|x| parse_named(x.into_inner()))
                .try_collect()?;

            let version = params
                .remove("version")
                .map(|v| v.try_cast(|i| i.parse_version(), None, "semver version number string"))
                .transpose()?;

            let other = params
                .into_iter()
                .flat_map(|(key, value)| match value.kind {
                    ExprKind::Ident(value) => Some((key, value.to_string())),
                    _ => None,
                })
                .collect();

            StmtKind::QueryDef(QueryDef { version, other })
        }
        Rule::func_def => {
            let mut pairs = pair.into_inner();
            let name = pairs.next().unwrap();
            let params = pairs.next().unwrap();
            let body = pairs.next().unwrap();

            let (name, return_type, _) = parse_typed_ident(name)?;

            let params: Vec<_> = params
                .into_inner()
                .into_iter()
                .map(parse_typed_ident)
                .try_collect()?;

            let mut positional_params = vec![];
            let mut named_params = vec![];
            for (name, ty, default_value) in params {
                let param = FuncParam {
                    name,
                    ty,
                    default_value,
                };
                if param.default_value.is_some() {
                    named_params.push(param)
                } else {
                    positional_params.push(param)
                }
            }

            StmtKind::FuncDef(FuncDef {
                name,
                positional_params,
                named_params,
                body: Box::from(expr_of_parse_pair(body)?),
                return_ty: return_type,
            })
        }
        Rule::var_def => {
            let mut pairs = pair.into_inner();

            let name = parse_ident_part(pairs.next().unwrap());
            let value = Box::new(expr_of_parse_pair(pairs.next().unwrap())?);

            StmtKind::VarDef(VarDef { name, value })
        }
        _ => unreachable!("{pair}"),
    };
    let mut stmt = Stmt::from(kind);
    stmt.span = Some(Span {
        start: span.start(),
        end: span.end(),
    });
    Ok(stmt)
}

/// Parses a parse tree of pest Pairs into an AST.
#[allow(dead_code)]
fn exprs_of_parse_pairs(pairs: Pairs<Rule>) -> Result<Vec<Expr>> {
    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(expr_of_parse_pair)
        .collect()
}

#[allow(dead_code)]
fn expr_of_parse_pair(pair: Pair<Rule>) -> Result<Expr> {
    let span = pair.as_span();
    let rule = pair.as_rule();
    let mut alias = None;

    let kind = match rule {
        Rule::list => ExprKind::List(exprs_of_parse_pairs(pair.into_inner())?),
        Rule::expr_mul | Rule::expr_add | Rule::expr_compare | Rule::expr => {
            let mut pairs = pair.into_inner();

            let mut expr = expr_of_parse_pair(pairs.next().unwrap())?;
            if let Some(op) = pairs.next() {
                let op = BinOp::from_str(op.as_str())?;

                expr = Expr::from(ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(expr_of_parse_pair(pairs.next().unwrap())?),
                });
            }

            expr.kind
        }
        Rule::expr_unary => {
            let mut pairs = pair.into_inner();

            let op = pairs.next().unwrap();

            let expr = expr_of_parse_pair(pairs.next().unwrap())?;
            let op = UnOp::from_str(op.as_str()).unwrap();
            match op {
                UnOp::Add => expr.kind,
                _ => ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
            }
        }

        // With coalesce, we need to grab the left and the right,
        // because we're transforming it into a function call rather
        // than passing along the operator. So this is unlike the rest
        // of the parsing (and maybe isn't optimal).
        Rule::expr_coalesce => {
            let mut pairs = pair.into_inner();
            let left = expr_of_parse_pair(pairs.next().unwrap())?;

            if pairs.next().is_none() {
                // If there's no coalescing, just return the single expression.
                left.kind
            } else {
                let right = expr_of_parse_pair(pairs.next().unwrap())?;
                ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::Coalesce,
                    right: Box::new(right),
                }
            }
        }
        // This makes the previous parsing a bit easier, but is hacky;
        // ideally find a better way (but it doesn't seem that easy to
        // parse parts of a Pairs).
        Rule::operator_coalesce => ExprKind::Ident(Ident::from_name("-")),

        Rule::assign | Rule::assign_call => {
            let (a, expr) = parse_named(pair.into_inner())?;
            alias = Some(a);
            expr.kind
        }
        Rule::func_call => {
            let mut pairs = pair.into_inner();

            let name = expr_of_parse_pair(pairs.next().unwrap())?;

            let mut named = HashMap::new();
            let mut positional = Vec::new();
            for arg in pairs {
                match arg.as_rule() {
                    Rule::named_arg => {
                        let (a, expr) = parse_named(arg.into_inner())?;
                        named.insert(a, expr);
                    }
                    _ => {
                        positional.push(expr_of_parse_pair(arg)?);
                    }
                }
            }

            ExprKind::FuncCall(FuncCall {
                name: Box::new(name),
                args: positional,
                named_args: named,
            })
        }
        Rule::jinja => {
            let inner = pair.as_str();
            ExprKind::Ident(Ident::from_name(inner))
        }
        Rule::ident => {
            // Pest has already parsed, so Chumsky should never fail
            panic!();
        }

        Rule::number => {
            // pest is responsible for ensuring these are in the correct place,
            // so we just need to remove them.
            let str = pair.as_str().replace('_', "");

            let lit = if let Ok(i) = str.parse::<i64>() {
                Literal::Integer(i)
            } else if let Ok(f) = str.parse::<f64>() {
                Literal::Float(f)
            } else {
                bail!("cannot parse {str} as number")
            };
            ExprKind::Literal(lit)
        }
        Rule::null => ExprKind::Literal(Literal::Null),
        Rule::boolean => ExprKind::Literal(Literal::Boolean(pair.as_str() == "true")),
        Rule::string => {
            // Takes the string_inner, without the quotes
            let inner = pair.into_inner().into_only();
            let inner = inner.map(|x| x.as_str().to_string()).unwrap_or_default();
            ExprKind::Literal(Literal::String(inner))
        }
        Rule::s_string => ExprKind::SString(ast_of_interpolate_items(pair)?),
        Rule::f_string => ExprKind::FString(ast_of_interpolate_items(pair)?),
        Rule::pipeline => {
            let mut nodes = exprs_of_parse_pairs(pair.into_inner())?;
            match nodes.len() {
                0 => unreachable!(),
                1 => nodes.remove(0).kind,
                _ => ExprKind::Pipeline(Pipeline { exprs: nodes }),
            }
        }
        Rule::nested_pipeline => {
            if let Some(pipeline) = pair.into_inner().next() {
                expr_of_parse_pair(pipeline)?.kind
            } else {
                ExprKind::Literal(Literal::Null)
            }
        }
        Rule::range => {
            let [start, end]: [Option<Box<Expr>>; 2] = pair
                .into_inner()
                // Iterate over `start` & `end` (separator is not a term).
                .into_iter()
                .map(|x| {
                    // Parse & Box each one.
                    exprs_of_parse_pairs(x.into_inner())
                        .and_then(|x| x.into_only())
                        .map(Box::new)
                        .ok()
                })
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|e| anyhow!("Expected start, separator, end; {e:?}"))?;
            ExprKind::Range(Range { start, end })
        }

        Rule::value_and_unit => {
            let pairs: Vec<_> = pair.into_inner().into_iter().collect();
            let [n, unit]: [Pair<Rule>; 2] = pairs
                .try_into()
                .map_err(|e| anyhow!("Expected two items; {e:?}"))?;

            ExprKind::Literal(Literal::ValueAndUnit(ValueAndUnit {
                n: n.as_str().parse()?,
                unit: unit.as_str().to_owned(),
            }))
        }

        Rule::date | Rule::time | Rule::timestamp => {
            let inner = pair.into_inner().into_only()?.as_str().to_string();

            ExprKind::Literal(match rule {
                Rule::date => Literal::Date(inner),
                Rule::time => Literal::Time(inner),
                Rule::timestamp => Literal::Timestamp(inner),
                _ => unreachable!(),
            })
        }

        Rule::switch => {
            let cases = pair
                .into_inner()
                .map(|pair| -> Result<_> {
                    let [condition, value]: [Expr; 2] = pair
                        .into_inner()
                        .map(expr_of_parse_pair)
                        .collect::<Result<Vec<_>>>()?
                        .try_into()
                        .unwrap();
                    Ok(SwitchCase { condition, value })
                })
                .try_collect()?;

            ExprKind::Switch(cases)
        }

        _ => unreachable!("{pair}"),
    };
    Ok(Expr {
        span: Some(Span {
            start: span.start(),
            end: span.end(),
        }),
        alias,
        ..Expr::from(kind)
    })
}

fn type_of_parse_pair(pair: Pair<Rule>) -> Result<Ty> {
    let any_of_terms: Vec<_> = pair
        .into_inner()
        .into_iter()
        .map(|pair| -> Result<Ty> {
            let mut pairs = pair.into_inner();
            let name = parse_ident_part(pairs.next().unwrap());
            let typ = match TyLit::from_str(&name) {
                Ok(t) => Ty::from(t),
                Err(_) if name == "table" => Ty::Table(Frame::default()),
                Err(_) => {
                    eprintln!("named type: {}", name);
                    Ty::Named(name.to_string())
                }
            };

            let param = pairs.next().map(type_of_parse_pair).transpose()?;

            Ok(if let Some(param) = param {
                Ty::Parameterized(Box::new(typ), Box::new(param))
            } else {
                typ
            })
        })
        .try_collect()?;

    // is there only a single element?
    Ok(match <[_; 1]>::try_from(any_of_terms) {
        Ok([only]) => only,
        Err(many) => Ty::AnyOf(many),
    })
}

fn parse_typed_ident(pair: Pair<Rule>) -> Result<(String, Option<Ty>, Option<Expr>)> {
    let mut pairs = pair.into_inner();

    let name = parse_ident_part(pairs.next().unwrap());

    let mut ty = None;
    let mut default = None;
    for pair in pairs {
        if matches!(pair.as_rule(), Rule::type_def) {
            ty = Some(type_of_parse_pair(pair)?);
        } else {
            default = Some(expr_of_parse_pair(pair)?);
        }
    }

    Ok((name, ty, default))
}

fn parse_named(mut pairs: Pairs<Rule>) -> Result<(String, Expr)> {
    let alias = parse_ident_part(pairs.next().unwrap());
    let expr = expr_of_parse_pair(pairs.next().unwrap())?;

    Ok((alias, expr))
}

fn parse_ident_part(pair: Pair<Rule>) -> String {
    pair.into_inner().next().unwrap().as_str().to_string()
}

fn ast_of_interpolate_items(pair: Pair<Rule>) -> Result<Vec<InterpolateItem>> {
    pair.into_inner()
        .map(|x| {
            Ok(match x.as_rule() {
                Rule::interpolate_string_inner_literal => {
                    InterpolateItem::String(x.as_str().to_string())
                }
                Rule::double_open_bracket => InterpolateItem::String("{".to_string()),
                Rule::double_close_bracket => InterpolateItem::String("}".to_string()),
                _ => InterpolateItem::Expr(Box::new(expr_of_parse_pair(x)?)),
            })
        })
        .collect::<Result<_>>()
}

#[cfg(test)]
mod test {

    use super::*;
    use chumsky::{prelude::*, Parser};
    use insta::assert_yaml_snapshot;

    fn parse_expr(string: &str) -> Result<Expr, Vec<anyhow::Error>> {
        let tokens = Parser::parse(&lexer::lexer(), string)
            .map_err(|errs| errs.into_iter().map(|e| anyhow!(e)).collect_vec())?;

        dbg!(&tokens);

        let len = string.chars().count();
        let stream = Stream::from_iter(len..len + 1, tokens.into_iter());
        Parser::parse(&expr::expr_call().then_ignore(end()), stream)
            .map_err(|errs| errs.into_iter().map(|e| anyhow!(e)).collect_vec())
    }

    #[test]
    fn test_parse_pipeline_parse_tree() {
        assert_yaml_snapshot!(parse(
            // It's useful to have canonical examples rather than copy-pasting
            // everything, so we reference the prql file here. But a downside of
            // this implementation is: if there's an error in extracting the
            // example from the docs into the file specified here, this test
            // won't compile. Because `cargo insta test --accept` on the
            // workspace — which extracts the example — requires compiling this,
            // we can get stuck.
            //
            // Breaking out of that requires running this `cargo insta test
            // --accept` within `book`, and then running it on the workspace.
            // `task test-all` does this.
            //
            // If we change this, it would great if we can retain having
            // examples tested in the docs.
            &include_str!("../../../book/tests/prql/examples/variables-0.prql"),
        )
        .unwrap());
    }

    #[test]
    fn test_parse_take() {
        parse("take 10").unwrap();

        assert_yaml_snapshot!(parse(r#"take 10"#).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - take
              args:
                - Literal:
                    Integer: 10
        "###);

        assert_yaml_snapshot!(parse(r#"take 1..10"#).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - take
              args:
                - Range:
                    start:
                      Literal:
                        Integer: 1
                    end:
                      Literal:
                        Integer: 10
        "###);
    }

    #[test]
    fn test_parse_ranges() {
        assert_yaml_snapshot!(parse_expr(r#"3..5"#).unwrap(), @r###"
        ---
        Range:
          start:
            Literal:
              Integer: 3
          end:
            Literal:
              Integer: 5
        "###);

        assert_yaml_snapshot!(parse_expr(r#"-2..-5"#).unwrap(), @r###"
        ---
        Range:
          start:
            Unary:
              op: Neg
              expr:
                Literal:
                  Integer: 2
          end:
            Unary:
              op: Neg
              expr:
                Literal:
                  Integer: 5
        "###);

        assert_yaml_snapshot!(parse_expr(r#"(-2..(-5 | abs))"#).unwrap(), @r###"
        ---
        Range:
          start:
            Unary:
              op: Neg
              expr:
                Literal:
                  Integer: 2
          end:
            Pipeline:
              exprs:
                - Unary:
                    op: Neg
                    expr:
                      Literal:
                        Integer: 5
                - Ident:
                    - abs
        "###);

        assert_yaml_snapshot!(parse_expr(r#"(2 + 5)..'a'"#).unwrap(), @r###"
        ---
        Range:
          start:
            Binary:
              left:
                Literal:
                  Integer: 2
              op: Add
              right:
                Literal:
                  Integer: 5
          end:
            Literal:
              String: a
        "###);

        assert_yaml_snapshot!(parse_expr(r#"1.6..rel.col"#).unwrap(), @r###"
        ---
        Range:
          start:
            Literal:
              Float: 1.6
          end:
            Ident:
              - rel
              - col
        "###);

        assert_yaml_snapshot!(parse_expr(r#"6.."#).unwrap(), @r###"
        ---
        Range:
          start:
            Literal:
              Integer: 6
          end: ~
        "###);
        assert_yaml_snapshot!(parse_expr(r#"..7"#).unwrap(), @r###"
        ---
        Range:
          start: ~
          end:
            Literal:
              Integer: 7
        "###);

        assert_yaml_snapshot!(parse_expr(r#".."#).unwrap(), @r###"
        ---
        Range:
          start: ~
          end: ~
        "###);

        assert_yaml_snapshot!(parse_expr(r#"@2020-01-01..@2021-01-01"#).unwrap(), @r###"
        ---
        Range:
          start:
            Literal:
              Date: 2020-01-01
          end:
            Literal:
              Date: 2021-01-01
        "###);
    }

    #[test]
    fn test_parse_basic_exprs() {
        assert_yaml_snapshot!(parse_expr(r#"country == "USA""#).unwrap(), @r###"
        ---
        Binary:
          left:
            Ident:
              - country
          op: Eq
          right:
            Literal:
              String: USA
        "###);
        assert_yaml_snapshot!(parse_expr("select [a, b, c]").unwrap(), @r###"
        ---
        FuncCall:
          name:
            Ident:
              - select
          args:
            - List:
                - Ident:
                    - a
                - Ident:
                    - b
                - Ident:
                    - c
        "###);
        assert_yaml_snapshot!(parse_expr(
            "group [title, country] (
                aggregate [sum salary]
            )"
        ).unwrap(), @r###"
        ---
        FuncCall:
          name:
            Ident:
              - group
          args:
            - List:
                - Ident:
                    - title
                - Ident:
                    - country
            - FuncCall:
                name:
                  Ident:
                    - aggregate
                args:
                  - List:
                      - FuncCall:
                          name:
                            Ident:
                              - sum
                          args:
                            - Ident:
                                - salary
        "###);
        assert_yaml_snapshot!(parse_expr(
            r#"    filter country == "USA""#
        ).unwrap(), @r###"
        ---
        FuncCall:
          name:
            Ident:
              - filter
          args:
            - Binary:
                left:
                  Ident:
                    - country
                op: Eq
                right:
                  Literal:
                    String: USA
        "###);
        assert_yaml_snapshot!(parse_expr("[a, b, c,]").unwrap(), @r###"
        ---
        List:
          - Ident:
              - a
          - Ident:
              - b
          - Ident:
              - c
        "###);
        assert_yaml_snapshot!(parse_expr(
            r#"[
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost
]"#
        ).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Ident:
                  - salary
              op: Add
              right:
                Ident:
                  - payroll_tax
            alias: gross_salary
          - Binary:
              left:
                Ident:
                  - gross_salary
              op: Add
              right:
                Ident:
                  - benefits_cost
            alias: gross_cost
        "###);
        // Currently not putting comments in our parse tree, so this is blank.
        assert_yaml_snapshot!(parse(
            r#"# this is a comment
        select a"#
        ).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - select
              args:
                - Ident:
                    - a
        "###);
        assert_yaml_snapshot!(parse_expr(
            "join side:left country [id==employee_id]"
        ).unwrap(), @r###"
        ---
        FuncCall:
          name:
            Ident:
              - join
          args:
            - Ident:
                - country
            - List:
                - Binary:
                    left:
                      Ident:
                        - id
                    op: Eq
                    right:
                      Ident:
                        - employee_id
          named_args:
            side:
              Ident:
                - left
        "###);
        assert_yaml_snapshot!(parse_expr("1  + 2").unwrap(), @r###"
        ---
        Binary:
          left:
            Literal:
              Integer: 1
          op: Add
          right:
            Literal:
              Integer: 2
        "###)
    }

    #[test]
    fn test_parse_string() {
        let double_quoted_ast = parse_expr(r#"" U S A ""#).unwrap();
        assert_yaml_snapshot!(double_quoted_ast, @r###"
        ---
        Literal:
          String: " U S A "
        "###);

        let single_quoted_ast = parse_expr(r#"' U S A '"#).unwrap();
        assert_eq!(single_quoted_ast, double_quoted_ast);

        // Single quotes within double quotes should produce a string containing
        // the single quotes (and vice versa).
        assert_yaml_snapshot!(parse_expr(r#""' U S A '""#).unwrap(), @r###"
        ---
        Literal:
          String: "' U S A '"
        "###);
        assert_yaml_snapshot!(parse_expr(r#"'" U S A "'"#).unwrap(), @r###"
        ---
        Literal:
          String: "\" U S A \""
        "###);

        parse_expr(r#"" U S A"#).unwrap_err();
        parse_expr(r#"" U S A '"#).unwrap_err();

        let escaped_string = parse_expr(r#"" \nU S A ""#).unwrap();
        assert_yaml_snapshot!(escaped_string, @r###"
        ---
        Literal:
          String: " \nU S A "
        "###);
        let escaped_string = escaped_string
            .kind
            .as_literal()
            .unwrap()
            .as_string()
            .unwrap();
        assert_eq!(escaped_string, " \nU S A ");

        let multi_double = parse_expr(
            r#""""
''
Canada
"

""""#,
        )
        .unwrap();
        assert_yaml_snapshot!(multi_double, @r###"
        ---
        Literal:
          String: "\n''\nCanada\n\"\n\n"
        "###);

        let multi_single = parse_expr(
            r#"'''
Canada
"
"""

'''"#,
        )
        .unwrap();
        assert_yaml_snapshot!(multi_single, @r###"
        ---
        Literal:
          String: "\nCanada\n\"\n\"\"\"\n\n"
        "###);

        assert_yaml_snapshot!(
          parse_expr("''").unwrap(),
          @r###"
        ---
        Literal:
          String: ""
        "###);
    }

    #[test]
    fn test_parse_s_string() {
        assert_yaml_snapshot!(parse_expr(r#"s"SUM({col})""#).unwrap(), @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Ident:
                - col
          - String: )
        "###);
        assert_yaml_snapshot!(parse_expr(r#"s"SUM({2 + 2})""#).unwrap(), @r###"
        ---
        SString:
          - String: SUM(
          - Expr:
              Binary:
                left:
                  Literal:
                    Integer: 2
                op: Add
                right:
                  Literal:
                    Integer: 2
          - String: )
        "###)
    }

    #[test]
    fn test_parse_s_string_braces() {
        assert_yaml_snapshot!(parse_expr(r#"s"{{?crystal_var}}""#).unwrap(), @r###"
        ---
        SString:
          - String: "{"
          - String: "?crystal_var"
          - String: "}"
        "###);
        assert_yaml_snapshot!(parse_expr(r#"s"foo{{bar""#).unwrap(), @r###"
        ---
        SString:
          - String: foo
          - String: "{"
          - String: bar
        "###);
    }

    #[test]
    fn test_parse_jinja() {
        assert_yaml_snapshot!(parse(r#"
        from {{ ref('stg_orders') }}
        aggregate (sum order_id)
        "#).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - "{{ ref('stg_orders') }}"
                - FuncCall:
                    name:
                      Ident:
                        - aggregate
                    args:
                      - FuncCall:
                          name:
                            Ident:
                              - sum
                          args:
                            - Ident:
                                - order_id
        "###)
    }

    #[test]
    fn test_parse_list() {
        assert_yaml_snapshot!(parse_expr(r#"[1 + 1, 2]"#).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                Literal:
                  Integer: 1
          - Literal:
              Integer: 2
        "###);
        assert_yaml_snapshot!(parse_expr(r#"[1 + (f 1), 2]"#).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                FuncCall:
                  name:
                    Ident:
                      - f
                  args:
                    - Literal:
                        Integer: 1
          - Literal:
              Integer: 2
        "###);
        // Line breaks
        assert_yaml_snapshot!(parse_expr(
            r#"[1,

                2]"#
        ).unwrap(), @r###"
        ---
        List:
          - Literal:
              Integer: 1
          - Literal:
              Integer: 2
        "###);
        // Function call in a list
        let ab = parse_expr(r#"[a b]"#).unwrap();
        let a_comma_b = parse_expr(r#"[a, b]"#).unwrap();
        assert_yaml_snapshot!(ab, @r###"
        ---
        List:
          - FuncCall:
              name:
                Ident:
                  - a
              args:
                - Ident:
                    - b
        "###);
        assert_yaml_snapshot!(a_comma_b, @r###"
        ---
        List:
          - Ident:
              - a
          - Ident:
              - b
        "###);
        assert_ne!(ab, a_comma_b);

        assert_yaml_snapshot!(parse_expr(r#"[amount, +amount, -amount]"#).unwrap(), @r###"
        ---
        List:
          - Ident:
              - amount
          - Unary:
              op: Add
              expr:
                Ident:
                  - amount
          - Unary:
              op: Neg
              expr:
                Ident:
                  - amount
        "###);
        // Operators in list items
        assert_yaml_snapshot!(parse_expr(r#"[amount, +amount, -amount]"#).unwrap(), @r###"
        ---
        List:
          - Ident:
              - amount
          - Unary:
              op: Add
              expr:
                Ident:
                  - amount
          - Unary:
              op: Neg
              expr:
                Ident:
                  - amount
        "###);
    }

    #[test]
    fn test_parse_number() {
        assert_yaml_snapshot!(parse_expr(r#"23"#).unwrap(), @r###"
        ---
        Literal:
          Integer: 23
        "###);
        assert_yaml_snapshot!(parse_expr(r#"2_3_4.5_6"#).unwrap(), @r###"
        ---
        Literal:
          Float: 234.56
        "###);
        assert_yaml_snapshot!(parse_expr(r#"23.6"#).unwrap(), @r###"
        ---
        Literal:
          Float: 23.6
        "###);
        assert_yaml_snapshot!(parse_expr(r#"23.0"#).unwrap(), @r###"
        ---
        Literal:
          Float: 23
        "###);
        assert_yaml_snapshot!(parse_expr(r#"2 + 2"#).unwrap(), @r###"
        ---
        Binary:
          left:
            Literal:
              Integer: 2
          op: Add
          right:
            Literal:
              Integer: 2
        "###);

        // Underscores at the beginning are parsed as ident
        parse_expr("_2").unwrap().kind.into_ident().unwrap();
        parse_expr("_").unwrap().kind.into_ident().unwrap();

        parse_expr("_2.3").unwrap_err();
        // expr_of_string("2_").unwrap_err(); // TODO
        // expr_of_string("2.3_").unwrap_err(); // TODO
    }

    #[test]
    fn test_parse_filter() {
        assert_yaml_snapshot!(
            parse(r#"filter country == "USA""#).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - filter
              args:
                - Binary:
                    left:
                      Ident:
                        - country
                    op: Eq
                    right:
                      Literal:
                        String: USA
        "###);

        assert_yaml_snapshot!(
            parse(r#"filter (upper country) == "USA""#).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - filter
              args:
                - Binary:
                    left:
                      FuncCall:
                        name:
                          Ident:
                            - upper
                        args:
                          - Ident:
                              - country
                    op: Eq
                    right:
                      Literal:
                        String: USA
        "###
        );
    }

    #[test]
    fn test_parse_aggregate() {
        let aggregate = parse(
            r"group [title] (
                aggregate [sum salary, count]
              )",
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - group
              args:
                - List:
                    - Ident:
                        - title
                - FuncCall:
                    name:
                      Ident:
                        - aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  - sum
                              args:
                                - Ident:
                                    - salary
                          - Ident:
                              - count
        "###);
        let aggregate = parse(
            r"group [title] (
                aggregate [sum salary]
              )",
        )
        .unwrap();
        assert_yaml_snapshot!(
            aggregate, @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - group
              args:
                - List:
                    - Ident:
                        - title
                - FuncCall:
                    name:
                      Ident:
                        - aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  - sum
                              args:
                                - Ident:
                                    - salary
        "###);
    }

    #[test]
    fn test_parse_derive() {
        assert_yaml_snapshot!(
            parse_expr(r#"derive [x = 5, y = (-x)]"#).unwrap()
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              - derive
          args:
            - List:
                - Literal:
                    Integer: 5
                  alias: x
                - Unary:
                    op: Neg
                    expr:
                      Ident:
                        - x
                  alias: y
        "###);
    }

    #[test]
    fn test_parse_select() {
        assert_yaml_snapshot!(
            parse_expr(r#"select x"#).unwrap()
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              - select
          args:
            - Ident:
                - x
        "###);

        assert_yaml_snapshot!(
            parse_expr(r#"select ![x]"#).unwrap()
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              - select
          args:
            - Unary:
                op: Not
                expr:
                  List:
                    - Ident:
                        - x
        "###);

        assert_yaml_snapshot!(
            parse_expr(r#"select [x, y]"#).unwrap()
        , @r###"
        ---
        FuncCall:
          name:
            Ident:
              - select
          args:
            - List:
                - Ident:
                    - x
                - Ident:
                    - y
        "###);
    }

    #[test]
    fn test_parse_expr() {
        assert_yaml_snapshot!(
            parse_expr(r#"country == "USA""#).unwrap()
        , @r###"
        ---
        Binary:
          left:
            Ident:
              - country
          op: Eq
          right:
            Literal:
              String: USA
        "###);
        assert_yaml_snapshot!(parse_expr(
                r#"[
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost,
]"#).unwrap(), @r###"
        ---
        List:
          - Binary:
              left:
                Ident:
                  - salary
              op: Add
              right:
                Ident:
                  - payroll_tax
            alias: gross_salary
          - Binary:
              left:
                Ident:
                  - gross_salary
              op: Add
              right:
                Ident:
                  - benefits_cost
            alias: gross_cost
        "###);
        assert_yaml_snapshot!(
            parse_expr(
                "(salary + payroll_tax) * (1 + tax_rate)"
            ).unwrap(),
            @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Ident:
                  - salary
              op: Add
              right:
                Ident:
                  - payroll_tax
          op: Mul
          right:
            Binary:
              left:
                Literal:
                  Integer: 1
              op: Add
              right:
                Ident:
                  - tax_rate
        "###)
    }

    #[test]
    fn test_parse_function() {
        assert_yaml_snapshot!(parse("func plus_one x ->  x + 1\n").unwrap(), @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    - x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);
        assert_yaml_snapshot!(parse("func identity x ->  x\n").unwrap()
        , @r###"
        ---
        - FuncDef:
            name: identity
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Ident:
                - x
            return_ty: ~
        "###);
        assert_yaml_snapshot!(parse("func plus_one x ->  (x + 1)\n").unwrap()
        , @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    - x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);
        assert_yaml_snapshot!(parse("func plus_one x ->  x + 1\n").unwrap()
        , @r###"
        ---
        - FuncDef:
            name: plus_one
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Binary:
                left:
                  Ident:
                    - x
                op: Add
                right:
                  Literal:
                    Integer: 1
            return_ty: ~
        "###);

        parse("func plus_one x:0 y:0 -> x + 1\n").unwrap_err();

        assert_yaml_snapshot!(parse("func foo x -> some_func (foo bar + 1) (plax) - baz\n").unwrap()
        , @r###"
        ---
        - FuncDef:
            name: foo
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              FuncCall:
                name:
                  Ident:
                    - some_func
                args:
                  - FuncCall:
                      name:
                        Ident:
                          - foo
                      args:
                        - Binary:
                            left:
                              Ident:
                                - bar
                            op: Add
                            right:
                              Literal:
                                Integer: 1
                  - Binary:
                      left:
                        Ident:
                          - plax
                      op: Sub
                      right:
                        Ident:
                          - baz
            return_ty: ~
        "###);

        assert_yaml_snapshot!(parse("func return_constant ->  42\n").unwrap(), @r###"
        ---
        - FuncDef:
            name: return_constant
            positional_params: []
            named_params: []
            body:
              Literal:
                Integer: 42
            return_ty: ~
        "###);

        assert_yaml_snapshot!(parse(r#"func count X -> s"SUM({X})"
        "#).unwrap(), @r###"
        ---
        - FuncDef:
            name: count
            positional_params:
              - name: X
                default_value: ~
            named_params: []
            body:
              SString:
                - String: SUM(
                - Expr:
                    Ident:
                      - X
                - String: )
            return_ty: ~
        "###);

        assert_yaml_snapshot!(parse(
            r#"
            func lag_day x ->  (
                window x
                by sec_id
                sort date
                lag 1
            )
        "#
        )
        .unwrap(), @r###"
        ---
        - FuncDef:
            name: lag_day
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident:
                          - window
                      args:
                        - Ident:
                            - x
                  - FuncCall:
                      name:
                        Ident:
                          - by
                      args:
                        - Ident:
                            - sec_id
                  - FuncCall:
                      name:
                        Ident:
                          - sort
                      args:
                        - Ident:
                            - date
                  - FuncCall:
                      name:
                        Ident:
                          - lag
                      args:
                        - Literal:
                            Integer: 1
            return_ty: ~
        "###);

        assert_yaml_snapshot!(parse("func add x to:a ->  x + to\n").unwrap(), @r###"
        ---
        - FuncDef:
            name: add
            positional_params:
              - name: x
                default_value: ~
            named_params:
              - name: to
                default_value:
                  Ident:
                    - a
            body:
              Binary:
                left:
                  Ident:
                    - x
                op: Add
                right:
                  Ident:
                    - to
            return_ty: ~
        "###);
    }

    #[test]
    fn test_parse_func_call() {
        // Function without argument
        let ast = parse_expr(r#"count"#).unwrap();
        let ident = ast.kind.into_ident().unwrap();
        assert_yaml_snapshot!(
            ident, @r###"
        ---
        - count
        "###);

        // A non-friendly option for #154
        let ast = parse_expr(r#"count s'*'"#).unwrap();
        let func_call: FuncCall = ast.kind.into_func_call().unwrap();
        assert_yaml_snapshot!(
            func_call, @r###"
        ---
        name:
          Ident:
            - count
        args:
          - SString:
              - String: "*"
        "###);

        let ast = parse_expr(r#"add bar to=3"#).unwrap();
        assert_yaml_snapshot!(
            ast, @r###"
        ---
        FuncCall:
          name:
            Ident:
              - add
          args:
            - Ident:
                - bar
            - Literal:
                Integer: 3
              alias: to
        "###);
    }

    #[test]
    fn test_parse_op_precedence() {
        assert_yaml_snapshot!(parse_expr(r#"1 + 2 - 3 - 4"#).unwrap(), @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Binary:
                  left:
                    Literal:
                      Integer: 1
                  op: Add
                  right:
                    Literal:
                      Integer: 2
              op: Sub
              right:
                Literal:
                  Integer: 3
          op: Sub
          right:
            Literal:
              Integer: 4
        "###);

        assert_yaml_snapshot!(parse_expr(r#"1 / 2 - 3 * 4 + 1"#).unwrap(), @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Binary:
                  left:
                    Literal:
                      Integer: 1
                  op: Div
                  right:
                    Literal:
                      Integer: 2
              op: Sub
              right:
                Binary:
                  left:
                    Literal:
                      Integer: 3
                  op: Mul
                  right:
                    Literal:
                      Integer: 4
          op: Add
          right:
            Literal:
              Integer: 1
        "###);

        assert_yaml_snapshot!(parse_expr(r#"a and b or c and d"#).unwrap(), @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Ident:
                  - a
              op: And
              right:
                Ident:
                  - b
          op: Or
          right:
            Binary:
              left:
                Ident:
                  - c
              op: And
              right:
                Ident:
                  - d
        "###);

        assert_yaml_snapshot!(parse_expr(r#"a and b + c or (d e) and f"#).unwrap(), @r###"
        ---
        Binary:
          left:
            Binary:
              left:
                Ident:
                  - a
              op: And
              right:
                Binary:
                  left:
                    Ident:
                      - b
                  op: Add
                  right:
                    Ident:
                      - c
          op: Or
          right:
            Binary:
              left:
                FuncCall:
                  name:
                    Ident:
                      - d
                  args:
                    - Ident:
                        - e
              op: And
              right:
                Ident:
                  - f
        "###);
    }

    #[test]
    fn test_parse_var_def() {
        assert_yaml_snapshot!(parse(
            "let newest_employees = (from employees)"
        ).unwrap(), @r###"
        ---
        - VarDef:
            name: newest_employees
            value:
              FuncCall:
                name:
                  Ident:
                    - from
                args:
                  - Ident:
                      - employees
        "###);

        assert_yaml_snapshot!(parse(
            r#"
        let newest_employees = (
          from employees
          group country (
            aggregate [
                average_country_salary = average salary
            ]
          )
          sort tenure
          take 50
        )"#.trim()).unwrap(),
         @r###"
        ---
        - VarDef:
            name: newest_employees
            value:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident:
                          - from
                      args:
                        - Ident:
                            - employees
                  - FuncCall:
                      name:
                        Ident:
                          - group
                      args:
                        - Ident:
                            - country
                        - FuncCall:
                            name:
                              Ident:
                                - aggregate
                            args:
                              - List:
                                  - FuncCall:
                                      name:
                                        Ident:
                                          - average
                                      args:
                                        - Ident:
                                            - salary
                                    alias: average_country_salary
                  - FuncCall:
                      name:
                        Ident:
                          - sort
                      args:
                        - Ident:
                            - tenure
                  - FuncCall:
                      name:
                        Ident:
                          - take
                      args:
                        - Literal:
                            Integer: 50
        "###);

        assert_yaml_snapshot!(parse(r#"
            let e = s"SELECT * FROM employees"
            "#).unwrap(), @r###"
        ---
        - VarDef:
            name: e
            value:
              SString:
                - String: SELECT * FROM employees
        "###);

        assert_yaml_snapshot!(parse(
          "let x = (

            from x_table

            select only_in_x = foo

          )

          from x"
        ).unwrap(), @r###"
        ---
        - VarDef:
            name: x
            value:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident:
                          - from
                      args:
                        - Ident:
                            - x_table
                  - FuncCall:
                      name:
                        Ident:
                          - select
                      args:
                        - Ident:
                            - foo
                          alias: only_in_x
        - Main:
            FuncCall:
              name:
                Ident:
                  - from
              args:
                - Ident:
                    - x
        "###);
    }

    #[test]
    fn test_inline_pipeline() {
        assert_yaml_snapshot!(parse_expr("(salary | percentile 50)").unwrap(), @r###"
        ---
        Pipeline:
          exprs:
            - Ident:
                - salary
            - FuncCall:
                name:
                  Ident:
                    - percentile
                args:
                  - Literal:
                      Integer: 50
        "###);
        assert_yaml_snapshot!(parse("func median x -> (x | percentile 50)\n").unwrap(), @r###"
        ---
        - FuncDef:
            name: median
            positional_params:
              - name: x
                default_value: ~
            named_params: []
            body:
              Pipeline:
                exprs:
                  - Ident:
                      - x
                  - FuncCall:
                      name:
                        Ident:
                          - percentile
                      args:
                        - Literal:
                            Integer: 50
            return_ty: ~
        "###);
    }

    #[test]
    fn test_parse_sql_parameters() {
        assert_yaml_snapshot!(parse(r#"
        from mytable
        filter [
          first_name == $1,
          last_name == $2.name
        ]
        "#).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - mytable
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - List:
                          - Binary:
                              left:
                                Ident:
                                  - first_name
                              op: Eq
                              right:
                                Ident:
                                  - $1
                          - Binary:
                              left:
                                Ident:
                                  - last_name
                              op: Eq
                              right:
                                Ident:
                                  - $2
                                  - name
        "###);
    }

    #[test]
    fn test_tab_characters() {
        // #284
        parse(
            "from c_invoice
join doc:c_doctype [==c_invoice_id]
select [
\tinvoice_no,
\tdocstatus
]",
        )
        .unwrap();
    }

    #[test]
    fn test_parse_backticks() {
        let prql = "
from `a/*.parquet`
aggregate [max c]
join `schema.table` [==id]
join `my-proj.dataset.table`
join `my-proj`.`dataset`.`table`
";

        assert_yaml_snapshot!(parse(prql).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - a/*.parquet
                - FuncCall:
                    name:
                      Ident:
                        - aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident:
                                  - max
                              args:
                                - Ident:
                                    - c
                - FuncCall:
                    name:
                      Ident:
                        - join
                    args:
                      - Ident:
                          - schema.table
                      - List:
                          - Unary:
                              op: EqSelf
                              expr:
                                Ident:
                                  - id
                - FuncCall:
                    name:
                      Ident:
                        - join
                    args:
                      - Ident:
                          - my-proj.dataset.table
                - FuncCall:
                    name:
                      Ident:
                        - join
                    args:
                      - Ident:
                          - my-proj
                          - dataset
                          - table
        "###);
    }

    #[test]
    fn test_parse_sort() {
        assert_yaml_snapshot!(parse("
        from invoices
        sort issued_at
        sort (-issued_at)
        sort [issued_at]
        sort [-issued_at]
        sort [issued_at, -amount, +num_of_articles]
        ").unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - invoices
                - FuncCall:
                    name:
                      Ident:
                        - sort
                    args:
                      - Ident:
                          - issued_at
                - FuncCall:
                    name:
                      Ident:
                        - sort
                    args:
                      - Unary:
                          op: Neg
                          expr:
                            Ident:
                              - issued_at
                - FuncCall:
                    name:
                      Ident:
                        - sort
                    args:
                      - List:
                          - Ident:
                              - issued_at
                - FuncCall:
                    name:
                      Ident:
                        - sort
                    args:
                      - List:
                          - Unary:
                              op: Neg
                              expr:
                                Ident:
                                  - issued_at
                - FuncCall:
                    name:
                      Ident:
                        - sort
                    args:
                      - List:
                          - Ident:
                              - issued_at
                          - Unary:
                              op: Neg
                              expr:
                                Ident:
                                  - amount
                          - Unary:
                              op: Add
                              expr:
                                Ident:
                                  - num_of_articles
        "###);
    }

    #[test]
    fn test_dates() {
        assert_yaml_snapshot!(parse("
        from employees
        derive [age_plus_two_years = (age + 2years)]
        ").unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - employees
                - FuncCall:
                    name:
                      Ident:
                        - derive
                    args:
                      - List:
                          - Binary:
                              left:
                                Ident:
                                  - age
                              op: Add
                              right:
                                Literal:
                                  ValueAndUnit:
                                    n: 2
                                    unit: years
                            alias: age_plus_two_years
        "###);

        assert_yaml_snapshot!(parse("
        derive [
            date = @2011-02-01,
            timestamp = @2011-02-01T10:00,
            time = @14:00,
            # datetime = @2011-02-01T10:00<datetime>,
        ]
        ").unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - derive
              args:
                - List:
                    - Literal:
                        Date: 2011-02-01
                      alias: date
                    - Literal:
                        Timestamp: "2011-02-01T10:00"
                      alias: timestamp
                    - Literal:
                        Time: "14:00"
                      alias: time
        "###);

        parse("derive x = @2020-01-0").unwrap_err();
    }

    #[test]
    fn test_multiline_string() {
        assert_yaml_snapshot!(parse(r###"
        derive x = r#"r-string test"#
        "###).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - derive
              args:
                - Ident:
                    - r
                  alias: x
        "### )
    }

    #[test]
    fn test_parse_coalesce() {
        assert_yaml_snapshot!(parse(r###"
        from employees
        derive amount = amount ?? 0
        "###).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - employees
                - FuncCall:
                    name:
                      Ident:
                        - derive
                    args:
                      - Binary:
                          left:
                            Ident:
                              - amount
                          op: Coalesce
                          right:
                            Literal:
                              Integer: 0
                        alias: amount
        "### )
    }

    #[test]
    fn test_parse_literal() {
        assert_yaml_snapshot!(parse(r###"
        derive x = true
        "###).unwrap(), @r###"
        ---
        - Main:
            FuncCall:
              name:
                Ident:
                  - derive
              args:
                - Literal:
                    Boolean: true
                  alias: x
        "###)
    }

    #[test]
    fn test_parse_allowed_idents() {
        assert_yaml_snapshot!(parse(r###"
        from employees
        join _salary [==employee_id] # table with leading underscore
        filter first_name == $1
        select [_employees._underscored_column]
        "###).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - employees
                - FuncCall:
                    name:
                      Ident:
                        - join
                    args:
                      - Ident:
                          - _salary
                      - List:
                          - Unary:
                              op: EqSelf
                              expr:
                                Ident:
                                  - employee_id
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              - first_name
                          op: Eq
                          right:
                            Ident:
                              - $1
                - FuncCall:
                    name:
                      Ident:
                        - select
                    args:
                      - List:
                          - Ident:
                              - _employees
                              - _underscored_column
        "###)
    }

    #[test]
    fn test_parse_gt_lt_gte_lte() {
        assert_yaml_snapshot!(parse(r###"
        from people
        filter age >= 100
        filter num_grandchildren <= 10
        filter salary > 0
        filter num_eyes < 2
        "###).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - people
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              - age
                          op: Gte
                          right:
                            Literal:
                              Integer: 100
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              - num_grandchildren
                          op: Lte
                          right:
                            Literal:
                              Integer: 10
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              - salary
                          op: Gt
                          right:
                            Literal:
                              Integer: 0
                - FuncCall:
                    name:
                      Ident:
                        - filter
                    args:
                      - Binary:
                          left:
                            Ident:
                              - num_eyes
                          op: Lt
                          right:
                            Literal:
                              Integer: 2
        "###)
    }

    #[test]
    fn test_assign() {
        assert_yaml_snapshot!(parse(r###"
from employees
join s=salaries [==id]
        "###).unwrap(), @r###"
        ---
        - Main:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident:
                        - from
                    args:
                      - Ident:
                          - employees
                - FuncCall:
                    name:
                      Ident:
                        - join
                    args:
                      - Ident:
                          - salaries
                        alias: s
                      - List:
                          - Unary:
                              op: EqSelf
                              expr:
                                Ident:
                                  - id
        "###);
    }
}
