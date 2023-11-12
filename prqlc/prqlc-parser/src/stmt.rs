use itertools::Itertools;
use std::collections::HashMap;

use chumsky::prelude::*;
use semver::VersionReq;

use prqlc_ast::expr::*;
use prqlc_ast::stmt::*;

use crate::types::type_expr;

use super::common::*;
use super::expr::*;
use super::lexer::Token;

pub fn source() -> impl Parser<Token, Vec<Stmt>, Error = PError> {
    query_def()
        .or_not()
        .chain(module_contents())
        .then_ignore(end())
}

fn module_contents() -> impl Parser<Token, Vec<Stmt>, Error = PError> {
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(module_contents.delimited_by(ctrl('{'), ctrl('}')))
            .map(|(name, stmts)| (Vec::new(), StmtKind::ModuleDef(ModuleDef { name, stmts })))
            .labelled("module definition");

        choice((type_def(), var_def(), module_def))
            .map_with_span(into_stmt)
            .separated_by(new_line().repeated().at_least(1))
            .allow_leading()
            .allow_trailing()
    })
}

fn query_def() -> impl Parser<Token, Stmt, Error = PError> {
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
                .map_err(|msg| Simple::custom(span, msg))?;

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
                .map_err(|msg| Simple::custom(span, msg))?
                .map_or_else(HashMap::new, |x| {
                    HashMap::from_iter(vec![("target".to_string(), x)])
                });

            if !args.is_empty() {
                return Err(Simple::custom(
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

fn var_def() -> impl Parser<Token, (Vec<Annotation>, StmtKind), Error = PError> {
    let annotation = just(Token::Annotate)
        .ignore_then(expr())
        .then_ignore(new_line().repeated())
        .map(|expr| Annotation {
            expr: Box::new(expr),
        });

    let let_ = keyword("let")
        .ignore_then(ident_part())
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then_ignore(ctrl('='))
        .then(expr_call().map(Box::new))
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
                value,
                ty: None,
            })
        })
        .labelled("variable definition");

    annotation.repeated().then(let_.or(main_or_into))
}

fn type_def() -> impl Parser<Token, (Vec<Annotation>, StmtKind), Error = PError> {
    keyword("type")
        .ignore_then(ident_part())
        .then(ctrl('=').ignore_then(type_expr()).or_not())
        .map(|(name, value)| (Vec::new(), StmtKind::TypeDef(TypeDef { name, value })))
        .labelled("type definition")
}
