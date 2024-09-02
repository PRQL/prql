use std::collections::HashMap;
use std::iter::zip;
use std::path::PathBuf;
use std::sync::OnceLock;

use itertools::Itertools;

use super::gen_expr::{translate_operand, ExprOrSource, SourceExpr};
use super::{Context, Dialect};
use crate::ir::{decl, pl, rq};
use crate::utils::Pluck;
use crate::Result;
use crate::{debug, semantic};
use crate::{Error, WithErrorInfo};

fn std() -> &'static decl::Module {
    static STD: OnceLock<decl::Module> = OnceLock::new();
    STD.get_or_init(|| {
        let _suppressed = debug::log_suppress();

        let std_lib = crate::SourceTree::new(
            [(
                PathBuf::from("std.sql.prql"),
                include_str!("./std.sql.prql").to_string(),
            )],
            None,
        );
        let ast = crate::parser::parse(&std_lib).unwrap();
        let context = semantic::resolve(ast).unwrap();

        context.module
    })
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
    let (operator_impl, binding_strength, window_frame, coalesce) =
        find_operator_impl(&name, ctx.dialect_enum).unwrap();
    let parent_binding_strength = binding_strength.unwrap_or(100);

    let args: HashMap<&str, _> = zip(operator_impl.params, args).collect();

    // body can only be an s-string
    let body = match &operator_impl.body.kind {
        pl::ExprKind::Literal(pl::Literal::Null) => {
            return Err(Error::new_simple(format!(
                "operator {} is not supported for dialect {}",
                name, ctx.dialect_enum
            )))
        }
        pl::ExprKind::SString(items) => items,
        _ => {
            return Err(Error::new_assert(
                "bad RQ operator implementation for SQL: expected function or a plain s-string",
            ))
        }
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
                    required_strength,
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

struct OperatorImpl<'a> {
    body: &'a pl::Expr,
    params: Vec<&'a str>,
}

fn find_operator_impl(
    operator_name: &str,
    dialect: Dialect,
) -> Option<(OperatorImpl<'_>, Option<i32>, bool, Option<String>)> {
    let operator_name = operator_name.strip_prefix("std.").unwrap();
    let operator_ident = pl::Ident::from_path(
        operator_name
            .split('.')
            .map(String::from)
            .collect::<Vec<_>>(),
    );

    let dialect_module = std().get(&pl::Ident::from_name(dialect.to_string()));

    let mut impl_decl = None;

    if let Some(dialect_module) = dialect_module {
        let module = dialect_module.kind.as_module().unwrap();
        impl_decl = module.get(&operator_ident);
    }

    if impl_decl.is_none() {
        impl_decl = std().get(&operator_ident);
    }

    let impl_decl = impl_decl?;

    let impl_expr = impl_decl.kind.as_expr().unwrap();
    let operator_impl = match &impl_expr.kind {
        pl::ExprKind::Func(func) => {
            let params: Vec<_> = func
                .named_params
                .iter()
                .chain(func.params.iter())
                .map(|x| x.name.split('.').last().unwrap_or(x.name.as_str()))
                .collect();
            OperatorImpl {
                body: func.body.as_ref(),
                params,
            }
        }
        _ => OperatorImpl {
            body: impl_expr.as_ref(),
            params: Vec::new(),
        },
    };

    let annotation = impl_decl.annotations.iter().exactly_one().ok();
    let mut annotation = annotation
        .and_then(|x| into_tuple_items(*x.expr.clone()).ok())
        .unwrap_or_default();

    let binding_strength = pluck_annotation(&mut annotation, "binding_strength")
        .and_then(|literal| literal.into_integer().ok())
        .map(|int| int as i32);

    let window_frame = pluck_annotation(&mut annotation, "window_frame")
        .and_then(|literal| literal.into_boolean().ok())
        .unwrap_or_default();

    let coalesce =
        pluck_annotation(&mut annotation, "coalesce").and_then(|val| val.into_string().ok());

    Some((operator_impl, binding_strength, window_frame, coalesce))
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

/// Find the items in a `@{a=b}`. We're only using annotations with tuples;
/// we can consider formalizing this constraint.
fn into_tuple_items(expr: pl::Expr) -> Result<Vec<(String, pl::ExprKind)>, pl::Expr> {
    match expr.kind {
        pl::ExprKind::Tuple(items) => items
            .into_iter()
            .map(|item| Ok((item.alias.clone().unwrap(), item.kind)))
            .collect(),
        _ => Err(expr),
    }
}
