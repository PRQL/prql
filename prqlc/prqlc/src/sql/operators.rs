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
                PathBuf::from("std.prql"),
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
    let Some((func_def, binding_strength, window_frame, coalesce)) =
        find_operator_impl(&name, ctx.dialect_enum)
    else {
        return Err(unsupported_operator(&name, ctx.dialect_enum));
    };
    let parent_binding_strength = binding_strength.unwrap_or(100);

    let params = func_def
        .named_params
        .iter()
        .chain(func_def.params.iter())
        .map(|x| x.name.split('.').next_back().unwrap_or(x.name.as_str()))
        .collect_vec();

    let param_count = params.len();
    let arg_count = args.len();

    let args: HashMap<&str, _> = zip(params, args).collect();

    // body can only be an s-string
    let body = match &func_def.body.kind {
        pl::ExprKind::Literal(pl::Literal::Null) => {
            return Err(unsupported_operator(&name, ctx.dialect_enum))
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
                //
                // A query may declare its own `internal std.<name>` taking
                // fewer arguments than the implementation's body reads, so
                // this can come up empty.
                let Some(arg) = args.get(ident.name.as_str()).cloned() else {
                    return Err(Error::new_simple(format!(
                        "operator {name} expects {param_count} arguments, found {arg_count}"
                    )));
                };

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

/// Raised both when the operator table has no entry for the dialect and when the
/// entry is a `null` body marking it explicitly unsupported.
fn unsupported_operator(name: &str, dialect: Dialect) -> Error {
    Error::new_simple(format!(
        "operator {name} is not supported for dialect {dialect}"
    ))
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

    let dialect_module = std().get(&pl::Ident::from_name(dialect.to_string()));

    let mut func_def = None;

    if let Some(dialect_module) = dialect_module {
        let module = dialect_module.kind.as_module().unwrap();
        func_def = module.get(&operator_ident);
    }

    if func_def.is_none() {
        func_def = std().get(&operator_ident);
    }

    let decl = func_def?;

    let func_def = decl.kind.as_expr().unwrap();
    let func_def = func_def.kind.as_func().unwrap();

    let annotation = decl.annotations.iter().exactly_one().ok();
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

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    /// `sql.redshift` maps date format specifiers like Postgres does, but had
    /// no `to_text` implementation to emit, so the operator lookup found
    /// nothing and panicked.
    #[test]
    fn redshift_date_to_text() {
        assert_snapshot!(crate::tests::compile(
            r#"
            prql target:sql.redshift
            from invoices
            select (invoice_date | date.to_text "%d/%m/%Y")
            "#
        ).unwrap(), @r"
        SELECT
          TO_CHAR(invoice_date, 'DD/MM/YYYY')
        FROM
          invoices
        ");
    }

    /// A query can declare its own `internal std.<name>`, so the operator
    /// lookup can come up empty on any target — which used to panic rather
    /// than report.
    #[test]
    fn unknown_internal_operator_is_reported() {
        assert_snapshot!(crate::tests::compile(
            r#"
            let my_op = column -> internal std.no_such_operator
            from invoices
            select (my_op total)
            "#
        ).unwrap_err(), @"
        Error:
           ╭─[ :4:21 ]
           │
         4 │             select (my_op total)
           │                     ─────┬─────
           │                          ╰─────── operator std.no_such_operator is not supported for dialect generic
        ───╯
        ");
    }

    /// A declaration's own parameter list doesn't have to match the arity of
    /// the operator it names, so the implementation's body can reference an
    /// argument that was never passed — which used to panic rather than
    /// report.
    #[test]
    fn internal_operator_arity_mismatch_is_reported() {
        assert_snapshot!(crate::tests::compile(
            r#"
            let my_op = column -> internal std.lag
            from invoices
            select (my_op total)
            "#
        ).unwrap_err(), @"
        Error:
           ╭─[ :4:21 ]
           │
         4 │             select (my_op total)
           │                     ─────┬─────
           │                          ╰─────── operator std.lag expects 2 arguments, found 1
        ───╯
        ");
    }
}
