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
        root_mod: semantic::Module::new_root(),
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

struct FunctionDecl<const ARG_COUNT: usize> {
    name: &'static str,
}

impl<const AC: usize> FunctionDecl<AC> {
    const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

// TODO: automatically generate these definitions
pub const STD_MUL: FunctionDecl<2> = FunctionDecl::new("std.mul");
pub const STD_DIV: FunctionDecl<2> = FunctionDecl::new("std.div");
pub const STD_MOD: FunctionDecl<2> = FunctionDecl::new("std.mod");
pub const STD_ADD: FunctionDecl<2> = FunctionDecl::new("std.add");
pub const STD_SUB: FunctionDecl<2> = FunctionDecl::new("std.sub");
pub const STD_EQ: FunctionDecl<2> = FunctionDecl::new("std.eq");
pub const STD_NE: FunctionDecl<2> = FunctionDecl::new("std.ne");
pub const STD_GT: FunctionDecl<2> = FunctionDecl::new("std.gt");
pub const STD_LT: FunctionDecl<2> = FunctionDecl::new("std.lt");
pub const STD_GTE: FunctionDecl<2> = FunctionDecl::new("std.gte");
pub const STD_LTE: FunctionDecl<2> = FunctionDecl::new("std.lte");
pub const STD_AND: FunctionDecl<2> = FunctionDecl::new("std.and");
pub const STD_OR: FunctionDecl<2> = FunctionDecl::new("std.or");
pub const STD_COALESCE: FunctionDecl<2> = FunctionDecl::new("std.coalesce");

pub fn try_unpack<const ARG_COUNT: usize>(
    expr: rq::Expr,
    decl: FunctionDecl<ARG_COUNT>,
) -> Result<Result<[rq::Expr; ARG_COUNT], rq::Expr>> {
    if let rq::ExprKind::BuiltInFunction { name, args } = &expr.kind {
        if decl.name == name {
            let (_, args) = expr.kind.into_built_in_function().unwrap();

            let args: [rq::Expr; ARG_COUNT] = args
                .try_into()
                .map_err(|_| anyhow::anyhow!("Bad usage of function {}", decl.name))?;

            return Ok(Ok(args));
        }
    }
    Ok(Err(expr))
}

pub fn try_unpack_with<const ARG_COUNT: usize, M, T>(
    expr: rq::Expr,
    decl: FunctionDecl<ARG_COUNT>,
    mapper: M,
) -> Result<Result<T, rq::Expr>>
where
    M: FnOnce([rq::Expr; ARG_COUNT]) -> Result<T, [rq::Expr; ARG_COUNT]>,
{
    if let rq::ExprKind::BuiltInFunction { name, args } = &expr.kind {
        if decl.name == name {
            let (name, args) = expr.kind.into_built_in_function().unwrap();

            let args: [rq::Expr; ARG_COUNT] = args
                .try_into()
                .map_err(|_| anyhow::anyhow!("Bad usage of function {}", decl.name))?;

            return Ok(match mapper(args) {
                Ok(res) => Ok(res),
                Err(args) => {
                    // mapper was unsuccessful, let's repack back the original Expr

                    let args = args.to_vec();
                    Err(rq::Expr {
                        kind: rq::ExprKind::BuiltInFunction { name, args },
                        ..expr
                    })
                }
            });
        }
    }
    Ok(Err(expr))
}
