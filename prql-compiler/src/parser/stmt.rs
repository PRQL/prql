use itertools::Itertools;
use std::collections::HashMap;

use chumsky::prelude::*;
use semver::VersionReq;

use crate::ast::pl::*;
use crate::semantic::NS_MAIN;
use crate::semantic::NS_QUERY_DEF;

use super::common::*;
use super::expr::*;
use super::lexer::Token;

pub fn source() -> impl Parser<Token, Vec<Stmt>, Error = Simple<Token>> {
    query_def()
        .or_not()
        .chain(module_contents())
        .then_ignore(end())
        .labelled("source file")
}

fn module_contents() -> impl Parser<Token, Vec<Stmt>, Error = Simple<Token>> {
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(module_contents.delimited_by(ctrl('{'), ctrl('}')))
            .map(|(name, stmts)| (name, StmtKind::ModuleDef(ModuleDef { stmts })))
            .labelled("module definition");

        choice((type_def(), var_def(), module_def))
            .map_with_span(into_stmt)
            .separated_by(new_line().repeated())
            .allow_leading()
            .allow_trailing()
    })
}

fn query_def() -> impl Parser<Token, Stmt, Error = Simple<Token>> {
    new_line()
        .repeated()
        .ignore_then(keyword("prql"))
        .ignore_then(
            // named arg
            ident_part().then_ignore(ctrl(':')).then(expr()).repeated(),
        )
        .then_ignore(new_line())
        .try_map(|args, span| {
            let mut args: HashMap<_, _> = args.into_iter().collect();

            let version = args
                .remove("version")
                .map(|v| match v.kind {
                    ExprKind::Literal(Literal::String(v)) => {
                        VersionReq::parse(&v).map_err(|e| e.to_string())
                    }
                    _ => Err("version must be a string literal".to_string()),
                })
                .transpose()
                .map_err(|msg| Simple::custom(span.clone(), msg))?;

            // TODO: `QueryDef` is currently implemented as `version` & `other`
            // fields. We want to raise an error if an unsupported field is
            // used, to avoid confusion (e.g. if someone passes `dialect`). So
            // at the moment we implement this as having a HashMap with 0 or 1
            // entries... We can decide how to implement `QueryDef` later, and
            // have this awkward construction in the meantime.
            let other = args
                .remove("target")
                .map(|v| match v.kind {
                    ExprKind::Ident(value) => Ok(value.to_string()),
                    _ => Err("target must be a string literal".to_string()),
                })
                .transpose()
                .map_err(|msg| Simple::custom(span.clone(), msg))?
                .map_or_else(HashMap::new, |x| {
                    HashMap::from_iter(vec![("target".to_string(), x)])
                });

            if !args.is_empty() {
                return Err(Simple::custom(
                    span,
                    format!(
                        "unknown query definition arguments {}",
                        args.keys()
                            .into_iter()
                            .map(|x| format!("`{}`", x))
                            .join(", ")
                    ),
                ));
            }

            Ok(StmtKind::QueryDef(QueryDef { version, other }))
        })
        .map(|kind| (NS_QUERY_DEF.to_string(), kind))
        .map_with_span(into_stmt)
        .labelled("query header")
}

fn var_def() -> impl Parser<Token, (String, StmtKind), Error = Simple<Token>> {
    let let_ = keyword("let")
        .ignore_then(ident_part())
        .then(type_expr().or_not())
        .then_ignore(ctrl('='))
        .then(expr_call().map(Box::new))
        .map(|((name, ty_expr), value)| {
            let stmt = StmtKind::VarDef(VarDef {
                value,
                ty_expr,
                kind: VarDefKind::Let,
            });
            (name, stmt)
        })
        .labelled("variable definition");

    let main_or_into = pipeline(expr_call())
        .map_with_span(into_expr)
        .map(Box::new)
        .then(keyword("into").ignore_then(ident_part()).or_not())
        .map(|(value, into)| {
            let (kind, name) = match into {
                None => (VarDefKind::Main, NS_MAIN.to_string()),
                Some(name) => (VarDefKind::Into, name),
            };
            let stmt = StmtKind::VarDef(VarDef {
                value,
                ty_expr: None,
                kind,
            });
            (name, stmt)
        })
        .labelled("variable definition");

    let_.or(main_or_into)
}

fn type_def() -> impl Parser<Token, (String, StmtKind), Error = Simple<Token>> {
    keyword("type")
        .ignore_then(ident_part())
        .then(
            ctrl('=')
                .ignore_then(expr_call())
                .or_not()
                .map(|value| TypeDef { value })
                .map(StmtKind::TypeDef),
        )
        .labelled("type definition")
}

pub fn type_expr() -> impl Parser<Token, Expr, Error = Simple<Token>> {
    let literal = select! { Token::Literal(lit) => ExprKind::Literal(lit) };

    let ident = ident().map(ExprKind::Ident);

    let term = literal.or(ident).map_with_span(into_expr);

    binary_op_parser(term, operator_or())
        .delimited_by(ctrl('<'), ctrl('>'))
        .labelled("type expression")
}
