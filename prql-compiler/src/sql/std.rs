use std::collections::HashMap;
use std::iter::zip;

use anyhow::Result;
use once_cell::sync::Lazy;
use sqlparser::ast::{self as sql_ast};

use super::gen_expr::translate_sstring;
use super::Context;
use crate::ast::{pl, rq};
use crate::semantic;

static STD: Lazy<semantic::Module> = Lazy::new(load_std_impl);

fn load_std_impl() -> semantic::Module {
    use crate::parser::parse;
    let std_lib = include_str!("./std_impl.prql");
    let statements = parse(std_lib).unwrap();

    let context = semantic::Context {
        root_mod: semantic::Module::new(),
        ..semantic::Context::default()
    };

    let (_, context) = semantic::resolve_only(statements, Some(context)).unwrap();
    let std = context.root_mod.get(&pl::Ident::from_name("std")).unwrap();

    std.kind.clone().into_module().unwrap()
}

pub(super) fn translate_built_in(
    name: String,
    args: Vec<rq::Expr>,
    ctx: &mut Context,
) -> Result<sql_ast::Expr> {
    let name = name.strip_prefix("std.").unwrap();

    let entry = STD.get(&pl::Ident::from_name(name)).unwrap();
    let func_def = entry.kind.as_func_def().unwrap();

    let params = func_def
        .named_params
        .iter()
        .chain(func_def.positional_params.iter())
        .map(|x| x.name.split('.').last().unwrap_or(x.name.as_str()));

    let mut args: HashMap<&str, _> = zip(params, args.into_iter()).collect();

    // body can only be an s-string
    let body = &func_def.body.kind.as_s_string().unwrap();
    let body = body
        .iter()
        .map(|item| {
            match item {
                pl::InterpolateItem::Expr(expr) => {
                    // s-string exprs can only contain idents
                    let ident = expr.kind.as_ident();
                    let ident = ident.as_ref().unwrap();

                    // lookup args
                    let arg = args.remove(ident.name.as_str());
                    pl::InterpolateItem::<rq::Expr>::Expr(Box::new(arg.unwrap()))
                }
                pl::InterpolateItem::String(s) => pl::InterpolateItem::String(s.clone()),
            }
        })
        .collect::<Vec<_>>();

    let s_string = translate_sstring(body, ctx)?;

    Ok(sql_ast::Expr::Identifier(sql_ast::Ident::new(s_string)))
}
