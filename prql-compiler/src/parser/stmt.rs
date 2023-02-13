use std::{collections::HashMap, str::FromStr};

use chumsky::prelude::*;
use semver::VersionReq;

use crate::ast::pl::*;

use super::common::*;
use super::expr::*;
use super::lexer::Token;

pub fn source() -> impl Parser<Token, Vec<Stmt>, Error = Simple<Token>> {
    query_def()
        .or_not()
        .chain::<Stmt, _, _>(
            var_def()
                .or(function_def())
                .separated_by(new_line().or(whitespace()).repeated())
                .allow_leading()
                .allow_trailing(),
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
    (new_line().or(whitespace()).repeated())
        .ignore_then(keyword("prql"))
        .ignore_then(
            // named arg
            whitespace()
                .ignore_then(ident_part())
                .then_ignore(ctrl(":").padded_by(whitespace().or_not()))
                .then(expr())
                .repeated(),
        )
        .then_ignore(whitespace().or_not().then(new_line()))
        .try_map(|args, span| {
            let mut args: HashMap<_, _> = args.into_iter().collect();

            let version = args
                .remove("version")
                .map(|v| match v.kind {
                    ExprKind::Literal(Literal::String(v)) => {
                        VersionReq::parse(&v).map_err(|e| e.to_string())
                    }
                    _ => Err("version must be a sting literal".to_string()),
                })
                .transpose()
                .map_err(|msg| Simple::custom(span, msg))?;

            let other = args
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
                .then(type_expr().or_not())
                .then_ignore(whitespace().or_not()),
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
                .then_ignore(whitespace().or_not())
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

        type_term
            .separated_by(ctrl("|").padded_by(whitespace().or_not()))
            .delimited_by(ctrl("<"), ctrl(">"))
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
