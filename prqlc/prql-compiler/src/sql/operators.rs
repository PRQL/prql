use std::collections::HashMap;
use std::iter::zip;
use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;

use super::gen_expr::{translate_operand, ExprOrSource, SourceExpr};
use super::{Context, Dialect};

use crate::ir::{decl, pl, rq};
use crate::semantic;
use crate::utils::Pluck;
use crate::{Error, WithErrorInfo};

static STD: Lazy<decl::Module> = Lazy::new(load_std_sql);

fn load_std_sql() -> decl::Module {
    let std_lib = crate::SourceTree::new([(
        PathBuf::from("std.prql"),
        include_str!("./std.sql.prql").to_string(),
    )]);
    let ast = crate::parser::parse(&std_lib).unwrap();

    let options = semantic::ResolverOptions {};

    let context = semantic::resolve(ast, options).unwrap();
    context.module
}

pub(super) fn translate_operator_expr(expr: rq::Expr, ctx: &mut Context) -> Result<ExprOrSource> {
    let (name, args) = expr.kind.into_operator().unwrap();

    let source = translate_operator(name, args, ctx).with_span(expr.span)?;

    Ok(ExprOrSource::Source(source))
}

pub(super) fn translate_operator(
    name: String,
    args: Vec<rq::Expr>,
    ctx: &mut Context,
) -> Result<SourceExpr> {
    let (func_def, binding_strength, window_frame, coalesce) =
        find_operator_impl(&name, ctx.dialect_enum).unwrap();
    let parent_binding_strength = binding_strength.unwrap_or(100);

    let params = func_def
        .named_params
        .iter()
        .chain(func_def.params.iter())
        .map(|x| x.name.split('.').last().unwrap_or(x.name.as_str()));

    let args: HashMap<&str, _> = zip(params, args).collect();

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

    let mut text = String::new();

    for item in body {
        match item {
            pl::InterpolateItem::Expr { expr, format } => {
                // s-string exprs can only contain idents
                let ident = expr.kind.as_ident();
                let ident = ident.as_ref().unwrap();

                // lookup args
                let arg = args.get(ident.name.as_str()).unwrap().clone();

                // binding strength
                let required_strength = format
                    .as_ref()
                    .and_then(|f| f.parse::<i32>().ok())
                    .unwrap_or(parent_binding_strength);

                // translate args
                let arg = translate_operand(
                    arg,
                    false,
                    // for recursive calls, we increase the binding strength of sub-expressions
                    // so that a / (b / c), translated as Expr { Expr a, /, Expr { Expr b, /, Expr c } }
                    // is translated as a / (b / c) instead of a / b / c
                    required_strength + 1,
                    super::gen_expr::Associativity::Both,
                    ctx,
                )?;

                text += &arg.into_source();
            }
            pl::InterpolateItem::String(s) => {
                text += s;
            }
        }
    }

    let mut binding_strength = parent_binding_strength;

    if !ctx.query.window_function {
        if let Some(default) = coalesce {
            text = format!("COALESCE({text}, {default})");
            binding_strength = 100;
        }
    }

    Ok(SourceExpr {
        text,
        binding_strength,
        window_frame,
    })
}

fn find_operator_impl(
    operator_name: &str,
    dialect: Dialect,
) -> Option<(&pl::Func, Option<i32>, bool, Option<String>)> {
    let operator_name = operator_name.strip_prefix("std.").unwrap();
    let operator_ident = pl::Ident::from_path(
        operator_name
            .split('.')
            .map(String::from)
            .collect::<Vec<_>>(),
    );

    let dialect_module = STD.get(&pl::Ident::from_name(dialect.to_string()));

    let mut func_def = None;

    if let Some(dialect_module) = dialect_module {
        let module = dialect_module.kind.as_module().unwrap();
        func_def = module.get(&operator_ident);
    }

    if func_def.is_none() {
        func_def = STD.get(&operator_ident);
    }

    let decl = func_def?;

    let func_def = decl.kind.as_expr().unwrap();
    let func_def = func_def.kind.as_func().unwrap();

    let mut annotation = decl
        .clone()
        .annotations
        .into_iter()
        .exactly_one()
        .ok()
        .and_then(|x| x.tuple_items().ok())
        .unwrap_or_default();

    let binding_strength = pluck_annotation(&mut annotation, "binding_strength")
        .and_then(|literal| literal.into_integer().ok())
        .map(|int| int as i32);

    let window_frame = pluck_annotation(&mut annotation, "window_frame")
        .and_then(|literal| literal.into_boolean().ok())
        .unwrap_or_default();

    let coalesce =
        pluck_annotation(&mut annotation, "coalesce").and_then(|val| val.into_string().ok());

    Some((func_def.as_ref(), binding_strength, window_frame, coalesce))
}

fn pluck_annotation(
    annotation: &mut Vec<(String, pl::ExprKind)>,
    name: &str,
) -> Option<pl::Literal> {
    annotation
        .pluck(|(n, val)| if n == name { Ok(val) } else { Err((n, val)) })
        .into_iter()
        .next()
        .and_then(|val| val.into_literal().ok())
}
