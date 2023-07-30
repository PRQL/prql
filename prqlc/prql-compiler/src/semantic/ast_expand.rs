use std::collections::HashMap;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use prqlc_ast::expr::{BinOp, BinaryExpr, Expr, ExprKind, Ident};
use prqlc_ast::stmt::{Annotation, Stmt, StmtKind, VarDefKind};

use crate::ir::pl::{self, new_binop};
use crate::semantic::{NS_THAT, NS_THIS};

/// An AST pass that maps AST to PL.
pub fn expand_expr(expr: Expr) -> Result<pl::Expr> {
    let kind = match expr.kind {
        ExprKind::Ident(v) => pl::ExprKind::Ident(v),
        ExprKind::Literal(v) => pl::ExprKind::Literal(v),
        ExprKind::Pipeline(v) => {
            let mut e = desugar_pipeline(v)?;
            e.alias = expr.alias.or(e.alias);
            return Ok(e);
        }
        ExprKind::Tuple(v) => pl::ExprKind::Tuple(expand_exprs(v)?),
        ExprKind::Array(v) => pl::ExprKind::Array(expand_exprs(v)?),
        ExprKind::Range(v) => pl::ExprKind::Range(v.try_map(expand_expr_box)?),

        ExprKind::Unary(unary) => expand_unary(unary)?,
        ExprKind::Binary(binary) => expand_binary(binary)?,

        ExprKind::FuncCall(v) => pl::ExprKind::FuncCall(pl::FuncCall {
            name: expand_expr_box(v.name)?,
            args: expand_exprs(v.args)?,
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| -> Result<_> { Ok((k, expand_expr(v)?)) })
                .try_collect()?,
        }),
        ExprKind::Func(v) => pl::ExprKind::Func(
            pl::Func {
                return_ty: v.return_ty.map(expand_ty_or_expr).transpose()?,
                body: expand_expr_box(v.body)?,
                params: expand_func_params(v.params)?,
                named_params: expand_func_params(v.named_params)?,
                name_hint: None,
                args: Vec::new(),
                env: HashMap::new(),
            }
            .into(),
        ),
        ExprKind::SString(v) => pl::ExprKind::SString(
            v.into_iter()
                .map(|v| v.try_map(expand_expr))
                .try_collect()?,
        ),
        ExprKind::FString(v) => pl::ExprKind::FString(
            v.into_iter()
                .map(|v| v.try_map(expand_expr))
                .try_collect()?,
        ),
        ExprKind::Case(v) => pl::ExprKind::Case(
            v.into_iter()
                .map(|case| -> Result<_> {
                    Ok(pl::SwitchCase {
                        condition: expand_expr_box(case.condition)?,
                        value: expand_expr_box(case.value)?,
                    })
                })
                .try_collect()?,
        ),
        ExprKind::Param(v) => pl::ExprKind::Param(v),
        ExprKind::Internal(v) => pl::ExprKind::Internal(v),
    };

    Ok(pl::Expr {
        kind,
        span: expr.span,
        alias: expr.alias,
        id: None,
        target_id: None,
        target_ids: Vec::new(),
        ty: None,
        lineage: None,
        needs_window: false,
        flatten: false,
    })
}

fn expand_exprs(exprs: Vec<prqlc_ast::expr::Expr>) -> Result<Vec<pl::Expr>> {
    exprs.into_iter().map(expand_expr).collect()
}

#[allow(clippy::boxed_local)]
fn expand_expr_box(expr: Box<prqlc_ast::expr::Expr>) -> Result<Box<pl::Expr>> {
    Ok(Box::new(expand_expr(*expr)?))
}

fn desugar_pipeline(mut pipeline: prqlc_ast::expr::Pipeline) -> Result<pl::Expr> {
    let value = pipeline.exprs.remove(0);
    let mut value = expand_expr(value)?;

    for expr in pipeline.exprs {
        let expr = expand_expr(expr)?;
        let span = expr.span;

        value = pl::Expr::new(pl::ExprKind::FuncCall(pl::FuncCall::new_simple(
            expr,
            vec![value],
        )));
        value.span = span;
    }

    Ok(value)
}

/// Desugar unary operators into function calls.
fn expand_unary(
    prqlc_ast::expr::UnaryExpr { op, expr }: prqlc_ast::expr::UnaryExpr,
) -> Result<pl::ExprKind> {
    use prqlc_ast::expr::UnOp::*;

    let expr = expand_expr(*expr)?;

    let func_name = match op {
        Neg => ["std", "neg"],
        Not => ["std", "not"],
        Add => return Ok(expr.kind),
        EqSelf => {
            let ident = expr.kind.into_ident().map_err(|_| {
                anyhow!("you can only use column names with self-equality operator.")
            })?;
            if !ident.path.is_empty() {
                return Err(anyhow!(
                    "you cannot use namespace prefix with self-equality operator."
                ));
            }
            let left = pl::Expr {
                span: expr.span,
                ..pl::Expr::new(Ident {
                    path: vec![NS_THIS.to_string()],
                    name: ident.name.clone(),
                })
            };
            let right = pl::Expr {
                span: expr.span,
                ..pl::Expr::new(Ident {
                    path: vec![NS_THAT.to_string()],
                    name: ident.name,
                })
            };
            return Ok(new_binop(left, &["std", "eq"], right).kind);
        }
    };
    Ok(pl::ExprKind::FuncCall(pl::FuncCall::new_simple(
        pl::Expr::new(Ident::from_path(func_name.to_vec())),
        vec![expr],
    )))
}

/// Desugar binary operators into function calls.
fn expand_binary(BinaryExpr { op, left, right }: BinaryExpr) -> Result<pl::ExprKind> {
    let left = expand_expr(*left)?;
    let right = expand_expr(*right)?;

    let func_name = match op {
        BinOp::Mul => ["std", "mul"],
        BinOp::DivInt => ["std", "div_i"],
        BinOp::DivFloat => ["std", "div_f"],
        BinOp::Mod => ["std", "mod"],
        BinOp::Add => ["std", "add"],
        BinOp::Sub => ["std", "sub"],
        BinOp::Eq => ["std", "eq"],
        BinOp::Ne => ["std", "ne"],
        BinOp::Gt => ["std", "gt"],
        BinOp::Lt => ["std", "lt"],
        BinOp::Gte => ["std", "gte"],
        BinOp::Lte => ["std", "lte"],
        BinOp::RegexSearch => ["std", "regex_search"],
        BinOp::And => ["std", "and"],
        BinOp::Or => ["std", "or"],
        BinOp::Coalesce => ["std", "coalesce"],
    };
    Ok(new_binop(left, &func_name, right).kind)
}

fn expand_func_param(value: prqlc_ast::expr::FuncParam) -> Result<pl::FuncParam> {
    Ok(pl::FuncParam {
        name: value.name,
        ty: value.ty.map(expand_ty_or_expr).transpose()?,
        default_value: value.default_value.map(expand_expr_box).transpose()?,
    })
}

fn expand_func_params(value: Vec<prqlc_ast::expr::FuncParam>) -> Result<Vec<pl::FuncParam>> {
    value.into_iter().map(expand_func_param).collect()
}

#[allow(clippy::boxed_local)]
fn expand_ty_or_expr(value: Box<prqlc_ast::expr::Expr>) -> Result<pl::TyOrExpr> {
    Ok(pl::TyOrExpr::Expr(Box::new(expand_expr(*value)?)))
}

fn expand_stmt(value: Stmt) -> Result<pl::Stmt> {
    Ok(pl::Stmt {
        id: None,
        kind: expand_stmt_kind(value.kind)?,
        span: value.span,
        annotations: value
            .annotations
            .into_iter()
            .map(expand_annotation)
            .try_collect()?,
    })
}

pub fn expand_stmts(value: Vec<Stmt>) -> Result<Vec<pl::Stmt>> {
    value.into_iter().map(expand_stmt).collect()
}

fn expand_stmt_kind(value: StmtKind) -> Result<pl::StmtKind> {
    Ok(match value {
        StmtKind::QueryDef(v) => pl::StmtKind::QueryDef(v),
        StmtKind::Main(v) => pl::StmtKind::VarDef(pl::VarDef {
            name: None,
            value: expand_expr_box(v)?,
            ty_expr: None,
            kind: pl::VarDefKind::Main,
        }),
        StmtKind::VarDef(v) => pl::StmtKind::VarDef(pl::VarDef {
            name: Some(v.name),
            value: expand_expr_box(v.value)?,
            ty_expr: v.ty_expr.map(expand_expr_box).transpose()?,
            kind: expand_var_def_kind(v.kind),
        }),
        StmtKind::TypeDef(v) => pl::StmtKind::TypeDef(pl::TypeDef {
            name: v.name,
            value: v.value.map(expand_expr_box).transpose()?,
        }),
        StmtKind::ModuleDef(v) => pl::StmtKind::ModuleDef(pl::ModuleDef {
            name: v.name,
            stmts: expand_stmts(v.stmts)?,
        }),
    })
}

fn expand_var_def_kind(value: VarDefKind) -> pl::VarDefKind {
    match value {
        VarDefKind::Let => pl::VarDefKind::Let,
        VarDefKind::Into => pl::VarDefKind::Into,
    }
}

fn expand_annotation(value: Annotation) -> Result<pl::Annotation> {
    Ok(pl::Annotation {
        expr: expand_expr_box(value.expr)?,
    })
}

/// An AST pass that tries to revert the mapping from AST to PL
pub fn restrict_expr(expr: pl::Expr) -> Expr {
    Expr {
        kind: restrict_expr_kind(expr.kind),
        span: expr.span,
        alias: expr.alias,
    }
}

#[allow(clippy::boxed_local)]
fn restrict_expr_box(expr: Box<pl::Expr>) -> Box<prqlc_ast::expr::Expr> {
    Box::new(restrict_expr(*expr))
}

fn restrict_exprs(exprs: Vec<pl::Expr>) -> Vec<Expr> {
    exprs.into_iter().map(restrict_expr).collect()
}

fn restrict_expr_kind(value: pl::ExprKind) -> ExprKind {
    match value {
        pl::ExprKind::Ident(v) => ExprKind::Ident(v),
        pl::ExprKind::Literal(v) => ExprKind::Literal(v),
        pl::ExprKind::Tuple(v) => ExprKind::Tuple(restrict_exprs(v)),
        pl::ExprKind::Array(v) => ExprKind::Array(restrict_exprs(v)),
        pl::ExprKind::Range(v) => ExprKind::Range(v.map(restrict_expr_box)),
        pl::ExprKind::FuncCall(v) => ExprKind::FuncCall(prqlc_ast::expr::FuncCall {
            name: restrict_expr_box(v.name),
            args: restrict_exprs(v.args),
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| (k, restrict_expr(v)))
                .collect(),
        }),
        pl::ExprKind::Func(v) => ExprKind::Func(
            prqlc_ast::expr::Func {
                return_ty: v.return_ty.map(restrict_ty_or_expr).map(Box::new),
                body: restrict_expr_box(v.body),
                params: restrict_func_params(v.params),
                named_params: restrict_func_params(v.named_params),
            }
            .into(),
        ),
        pl::ExprKind::SString(v) => {
            ExprKind::SString(v.into_iter().map(|v| v.map(restrict_expr)).collect())
        }
        pl::ExprKind::FString(v) => {
            ExprKind::FString(v.into_iter().map(|v| v.map(restrict_expr)).collect())
        }
        pl::ExprKind::Case(v) => ExprKind::Case(
            v.into_iter()
                .map(|case| prqlc_ast::expr::SwitchCase {
                    condition: restrict_expr_box(case.condition),
                    value: restrict_expr_box(case.value),
                })
                .collect(),
        ),
        pl::ExprKind::Param(v) => ExprKind::Param(v),
        pl::ExprKind::Internal(v) => ExprKind::Internal(v),

        // TODO: these are not correct, they are producing invalid PRQL
        pl::ExprKind::All { within, .. } => ExprKind::Ident(within),
        pl::ExprKind::Type(ty) => ExprKind::Ident(Ident::from_name(format!("<{}>", ty))),
        pl::ExprKind::TransformCall(tc) => ExprKind::Ident(Ident::from_name(format!(
            "({} ...)",
            tc.kind.as_ref().as_ref()
        ))),
        pl::ExprKind::RqOperator { name, .. } => {
            ExprKind::Ident(Ident::from_name(format!("({} ...)", name)))
        }
    }
}

fn restrict_func_params(value: Vec<pl::FuncParam>) -> Vec<prqlc_ast::expr::FuncParam> {
    value.into_iter().map(restrict_func_param).collect()
}

fn restrict_func_param(value: pl::FuncParam) -> prqlc_ast::expr::FuncParam {
    prqlc_ast::expr::FuncParam {
        name: value.name,
        ty: value.ty.map(restrict_ty_or_expr).map(Box::new),
        default_value: value.default_value.map(restrict_expr_box),
    }
}

fn restrict_ty_or_expr(value: pl::TyOrExpr) -> prqlc_ast::expr::Expr {
    match value {
        pl::TyOrExpr::Ty(ty) => restrict_ty(ty),
        pl::TyOrExpr::Expr(expr) => restrict_expr(*expr),
    }
}

fn restrict_ty(value: pl::Ty) -> prqlc_ast::expr::Expr {
    let expr_kind = match value.kind {
        pl::TyKind::Primitive(prim) => {
            ExprKind::Ident(Ident::from_path(vec!["std".to_string(), prim.to_string()]))
        }
        pl::TyKind::Singleton(lit) => ExprKind::Literal(lit),
        pl::TyKind::Union(mut variants) => {
            variants.reverse();
            let mut res = restrict_ty(variants.pop().unwrap().1);
            while let Some((_, ty)) = variants.pop() {
                let ty = restrict_ty(ty);
                res = Expr::new(ExprKind::Binary(BinaryExpr {
                    left: Box::new(res),
                    op: BinOp::Or,
                    right: Box::new(ty),
                }));
            }
            return res;
        }
        pl::TyKind::Tuple(fields) => ExprKind::Tuple(
            fields
                .into_iter()
                .map(|field| match field {
                    pl::TupleField::Single(name, ty) => {
                        // TODO: ty might be None
                        let mut e = restrict_ty(ty.unwrap());
                        if let Some(name) = name {
                            e.alias = Some(name);
                        }
                        e
                    }
                    pl::TupleField::Wildcard(_) => {
                        // TODO: this is not correct
                        Expr::new(ExprKind::Ident(Ident::from_name("*")))
                    }
                })
                .collect(),
        ),
        pl::TyKind::Array(item_ty) => ExprKind::Array(vec![restrict_ty(*item_ty)]),
        pl::TyKind::Set => todo!(),
        pl::TyKind::Function(_) => todo!(),
        pl::TyKind::Any => ExprKind::Ident(Ident::from_name("any")),
    };
    Expr::new(expr_kind)
}
