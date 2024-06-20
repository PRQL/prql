use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;
use prqlc_ast::expr::{ExprKind, IndirectionKind};
use prqlc_ast::stmt::{
    Annotation, ImportDef, ModuleDef, QueryDef, Stmt, StmtKind, TypeDef, VarDef, VarDefKind,
};
use prqlc_ast::token::{Literal, TokenKind};
use semver::VersionReq;

use super::common::{ctrl, ident_part, into_stmt, keyword, new_line};
use super::expr::{expr, expr_call, ident, pipeline};
use crate::types::type_expr;
use crate::{common::with_aesthetics, err::parse_error::PError};

pub fn source() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    with_aesthetics(query_def())
        .or_not()
        .chain(module_contents())
        .then_ignore(end())
}

fn module_contents() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(module_contents.delimited_by(ctrl('{'), ctrl('}')))
            .map(|(name, stmts)| StmtKind::ModuleDef(ModuleDef { name, stmts }))
            .labelled("module definition");

        let annotation = just(TokenKind::Annotate)
            .ignore_then(expr())
            .then_ignore(new_line().repeated())
            .map(|expr| Annotation {
                expr: Box::new(expr),
                aesthetics_before: Vec::new(),
                aesthetics_after: Vec::new(),
            });

        // TODO: I think some duplication here; we allow for potential
        // newlines before each item here, but then also have `.allow_leading`
        // below — since now we can get newlines after a comment between the
        // aesthetic item and the stmt... So a bit messy
        let stmt_kind = new_line().repeated().ignore_then(choice((
            module_def,
            type_def(),
            import_def(),
            var_def(),
        )));

        // Two wrapping of `with_aesthetics` — the first for the whole block,
        // and the second for just the annotation; if there's a comment between
        // the annotation and the code.
        with_aesthetics(
            with_aesthetics(annotation)
                .repeated()
                // TODO: do we need this? I think possibly we get an additional
                // error when we remove it; check (because it seems redundant...).
                .then_ignore(new_line().repeated())
                .then(stmt_kind)
                .map_with_span(into_stmt),
        )
        .separated_by(new_line().repeated().at_least(1))
        .allow_leading()
        .allow_trailing()
    })
}

fn query_def() -> impl Parser<TokenKind, Stmt, Error = PError> + Clone {
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
                .map_err(|msg| PError::custom(span, msg))?;

            // TODO: `QueryDef` is currently implemented as `version` & `other`
            // fields. We want to raise an error if an unsupported field is
            // used, to avoid confusion (e.g. if someone passes `dialect`). So
            // at the moment we implement this as having a HashMap with 0 or 1
            // entries... We can decide how to implement `QueryDef` later, and
            // have this awkward construction in the meantime.
            let other = args
                .remove("target")
                .map(|v| {
                    match v.kind {
                        ExprKind::Ident(name) => return Ok(name.to_string()),
                        ExprKind::Indirection {
                            base,
                            field: IndirectionKind::Name(field),
                        } => {
                            if let ExprKind::Ident(name) = base.kind {
                                return Ok(name.to_string() + "." + &field);
                            }
                        }
                        _ => {}
                    };
                    Err("target must be a string literal".to_string())
                })
                .transpose()
                .map_err(|msg| PError::custom(span, msg))?
                .map_or_else(HashMap::new, |x| {
                    HashMap::from_iter(vec![("target".to_string(), x)])
                });

            if !args.is_empty() {
                return Err(PError::custom(
                    span,
                    format!(
                        "unknown query definition arguments {}",
                        args.keys().map(|x| format!("`{}`", x)).join(", ")
                    ),
                ));
            }

            Ok(StmtKind::QueryDef(Box::new(QueryDef { version, other })))
        })
        .map(|kind| (Vec::new(), kind))
        .map_with_span(into_stmt)
        .labelled("query header")
}

fn var_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    let let_ = new_line()
        .repeated()
        .ignore_then(keyword("let"))
        .ignore_then(ident_part())
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then(ctrl('=').ignore_then(expr_call()).map(Box::new).or_not())
        .map(|((name, ty), value)| {
            StmtKind::VarDef(VarDef {
                name,
                value,
                ty,
                kind: VarDefKind::Let,
            })
        })
        .labelled("variable definition");

    let main_or_into = pipeline(expr_call())
        .map(Box::new)
        .then(keyword("into").ignore_then(ident_part()).or_not())
        .map(|(value, name)| {
            let kind = if name.is_none() {
                VarDefKind::Main
            } else {
                VarDefKind::Into
            };
            let name = name.unwrap_or_else(|| "main".to_string());

            StmtKind::VarDef(VarDef {
                name,
                kind,
                value: Some(value),
                ty: None,
            })
        })
        .labelled("variable definition");

    let_.or(main_or_into)
}

fn type_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    keyword("type")
        .ignore_then(ident_part())
        .then(ctrl('=').ignore_then(type_expr()).or_not())
        .map(|(name, value)| StmtKind::TypeDef(TypeDef { name, value }))
        .labelled("type definition")
}

fn import_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    keyword("import")
        .ignore_then(ident_part().then_ignore(ctrl('=')).or_not())
        .then(ident())
        .map(|(alias, name)| StmtKind::ImportDef(ImportDef { name, alias }))
        .labelled("import statement")
}
