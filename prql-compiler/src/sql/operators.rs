use std::collections::HashMap;
use std::iter::zip;
use std::path::PathBuf;

use anyhow::Result;
use once_cell::sync::Lazy;
use sqlparser::ast::{self as sql_ast};

use super::gen_expr::translate_sstring;
use super::{Context, Dialect};

use crate::ast::{pl, rq};
use crate::error::WithErrorInfo;
use crate::semantic;
use crate::Error;

static STD: Lazy<semantic::Module> = Lazy::new(load_std_sql);

fn load_std_sql() -> semantic::Module {
    let std_lib = crate::SourceTree::new([(
        PathBuf::from("std.prql"),
        include_str!("./std.sql.prql").to_string(),
    )]);
    let ast = crate::parser::parse(&std_lib).unwrap();

    let options = semantic::ResolverOptions {
        allow_module_decls: true,
    };

    let context = semantic::resolve(ast, options).unwrap();
    context.root_mod
}

pub(super) fn translate_operator_expr(expr: rq::Expr, ctx: &mut Context) -> Result<sql_ast::Expr> {
    let (name, args) = expr.kind.into_operator().unwrap();

    let s_string = translate_operator(name, args, ctx).with_span(expr.span)?;

    Ok(sql_ast::Expr::Identifier(sql_ast::Ident::new(s_string)))
}

pub(super) fn translate_operator(
    name: String,
    args: Vec<rq::Expr>,
    ctx: &mut Context,
) -> Result<String> {
    let func_def = find_operator_impl(&name, ctx.dialect_enum).unwrap();

    let params = func_def
        .named_params
        .iter()
        .chain(func_def.params.iter())
        .map(|x| x.name.split('.').last().unwrap_or(x.name.as_str()));

    let args: HashMap<&str, _> = zip(params, args.into_iter()).collect();

    // body can only be an s-string
    let body = match &func_def.body.kind {
        pl::ExprKind::Literal(pl::Literal::Null) => {
            return Err(Error::new_simple(format!(
                "operator {} is not supported for dialect {}",
                name, ctx.dialect_enum
            ))
            .into())
        }
        pl::ExprKind::SString(items) => items,
        _ => panic!("Bad RQ operator implementation. Expected s-string or null"),
    };
    let body = body
        .iter()
        .map(|item| {
            match item {
                pl::InterpolateItem::Expr(expr) => {
                    // s-string exprs can only contain idents
                    let ident = expr.kind.as_ident();
                    let ident = ident.as_ref().unwrap();

                    // lookup args
                    let arg = args.get(ident.name.as_str());
                    pl::InterpolateItem::<rq::Expr>::Expr(Box::new(arg.cloned().unwrap()))
                }
                pl::InterpolateItem::String(s) => pl::InterpolateItem::String(s.clone()),
            }
        })
        .collect::<Vec<_>>();

    translate_sstring(body, ctx)
}

fn find_operator_impl(operator_name: &str, dialect: Dialect) -> Option<&pl::Func> {
    let operator_name = operator_name.strip_prefix("std.").unwrap();

    let operator_name = pl::Ident::from_name(operator_name);

    let dialect_module = STD.get(&pl::Ident::from_name(dialect.to_string()));

    let mut func_def = None;

    if let Some(dialect_module) = dialect_module {
        let module = dialect_module.kind.as_module().unwrap();
        func_def = module.get(&operator_name);
    }

    if func_def.is_none() {
        func_def = STD.get(&operator_name);
    }

    let func_def = func_def?.kind.as_expr().unwrap();
    let func_def = func_def.kind.as_func().unwrap();
    Some(func_def.as_ref())
}
