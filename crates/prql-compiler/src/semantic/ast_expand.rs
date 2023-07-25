use std::collections::HashMap;

use prql_ast::expr::{BinOp, BinaryExpr, Expr, ExprKind, Ident};
use prql_ast::stmt::{Annotation, Stmt, StmtKind, VarDefKind};

use crate::ir::pl;

/// An AST pass that maps AST to PL.
pub fn expand_expr(expr: Expr) -> pl::Expr {
    pl::Expr {
        kind: expand_expr_kind(expr.kind),
        span: expr.span,
        alias: expr.alias,
        id: None,
        target_id: None,
        target_ids: Vec::new(),
        ty: None,
        lineage: None,
        needs_window: false,
        flatten: false,
    }
}

fn expand_exprs(exprs: Vec<prql_ast::expr::Expr>) -> Vec<pl::Expr> {
    exprs.into_iter().map(expand_expr).collect::<Vec<_>>()
}

#[allow(clippy::boxed_local)]
fn expand_expr_box(expr: Box<prql_ast::expr::Expr>) -> Box<pl::Expr> {
    Box::new(expand_expr(*expr))
}

fn expand_expr_kind(value: ExprKind) -> pl::ExprKind {
    match value {
        ExprKind::Ident(v) => pl::ExprKind::Ident(v),
        ExprKind::Literal(v) => pl::ExprKind::Literal(v),
        ExprKind::Pipeline(v) => pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: expand_exprs(v.exprs),
        }),
        ExprKind::Tuple(v) => pl::ExprKind::Tuple(expand_exprs(v)),
        ExprKind::Array(v) => pl::ExprKind::Array(expand_exprs(v)),
        ExprKind::Range(v) => pl::ExprKind::Range(v.map(expand_expr_box)),
        ExprKind::Binary(v) => pl::ExprKind::Binary(pl::BinaryExpr {
            left: expand_expr_box(v.left),
            op: v.op,
            right: expand_expr_box(v.right),
        }),
        ExprKind::Unary(v) => pl::ExprKind::Unary(pl::UnaryExpr {
            op: v.op,
            expr: expand_expr_box(v.expr),
        }),
        ExprKind::FuncCall(v) => pl::ExprKind::FuncCall(pl::FuncCall {
            name: expand_expr_box(v.name),
            args: expand_exprs(v.args),
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| (k, expand_expr(v)))
                .collect(),
        }),
        ExprKind::Func(v) => pl::ExprKind::Func(
            pl::Func {
                return_ty: v.return_ty.map(expand_ty_or_expr),
                body: expand_expr_box(v.body),
                params: expand_func_params(v.params),
                named_params: expand_func_params(v.named_params),
                name_hint: None,
                args: Vec::new(),
                env: HashMap::new(),
            }
            .into(),
        ),
        ExprKind::SString(v) => {
            pl::ExprKind::SString(v.into_iter().map(|v| v.map(expand_expr)).collect())
        }
        ExprKind::FString(v) => {
            pl::ExprKind::FString(v.into_iter().map(|v| v.map(expand_expr)).collect())
        }
        ExprKind::Case(v) => pl::ExprKind::Case(
            v.into_iter()
                .map(|case| pl::SwitchCase {
                    condition: expand_expr_box(case.condition),
                    value: expand_expr_box(case.value),
                })
                .collect(),
        ),
        ExprKind::Param(v) => pl::ExprKind::Param(v),
        ExprKind::Internal(v) => pl::ExprKind::Internal(v),
    }
}

fn expand_func_param(value: prql_ast::expr::FuncParam) -> pl::FuncParam {
    pl::FuncParam {
        name: value.name,
        ty: value.ty.map(expand_ty_or_expr),
        default_value: value.default_value.map(expand_expr_box),
    }
}

fn expand_func_params(value: Vec<prql_ast::expr::FuncParam>) -> Vec<pl::FuncParam> {
    value.into_iter().map(expand_func_param).collect::<Vec<_>>()
}

#[allow(clippy::boxed_local)]
fn expand_ty_or_expr(value: Box<prql_ast::expr::Expr>) -> pl::TyOrExpr {
    pl::TyOrExpr::Expr(Box::new(expand_expr(*value)))
}

fn expand_stmt(value: Stmt) -> pl::Stmt {
    pl::Stmt {
        id: None,
        kind: expand_stmt_kind(value.kind),
        span: value.span,
        annotations: value
            .annotations
            .into_iter()
            .map(expand_annotation)
            .collect(),
    }
}

pub fn expand_stmts(value: Vec<Stmt>) -> Vec<pl::Stmt> {
    value.into_iter().map(expand_stmt).collect()
}

fn expand_stmt_kind(value: StmtKind) -> pl::StmtKind {
    match value {
        StmtKind::QueryDef(v) => pl::StmtKind::QueryDef(v),
        StmtKind::Main(v) => pl::StmtKind::VarDef(pl::VarDef {
            name: None,
            value: expand_expr_box(v),
            ty_expr: None,
            kind: pl::VarDefKind::Main,
        }),
        StmtKind::VarDef(v) => pl::StmtKind::VarDef(pl::VarDef {
            name: Some(v.name),
            value: expand_expr_box(v.value),
            ty_expr: v.ty_expr.map(expand_expr_box),
            kind: expand_var_def_kind(v.kind),
        }),
        StmtKind::TypeDef(v) => pl::StmtKind::TypeDef(pl::TypeDef {
            name: v.name,
            value: v.value.map(expand_expr_box),
        }),
        StmtKind::ModuleDef(v) => pl::StmtKind::ModuleDef(pl::ModuleDef {
            name: v.name,
            stmts: expand_stmts(v.stmts),
        }),
    }
}

fn expand_var_def_kind(value: VarDefKind) -> pl::VarDefKind {
    match value {
        VarDefKind::Let => pl::VarDefKind::Let,
        VarDefKind::Into => pl::VarDefKind::Into,
    }
}

fn expand_annotation(value: Annotation) -> pl::Annotation {
    pl::Annotation {
        expr: expand_expr_box(value.expr),
    }
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
fn restrict_expr_box(expr: Box<pl::Expr>) -> Box<prql_ast::expr::Expr> {
    Box::new(restrict_expr(*expr))
}

fn restrict_exprs(exprs: Vec<pl::Expr>) -> Vec<Expr> {
    exprs.into_iter().map(restrict_expr).collect()
}

fn restrict_expr_kind(value: pl::ExprKind) -> ExprKind {
    match value {
        pl::ExprKind::Ident(v) => ExprKind::Ident(v),
        pl::ExprKind::Literal(v) => ExprKind::Literal(v),
        pl::ExprKind::Pipeline(v) => ExprKind::Pipeline(prql_ast::expr::Pipeline {
            exprs: restrict_exprs(v.exprs),
        }),
        pl::ExprKind::Tuple(v) => ExprKind::Tuple(restrict_exprs(v)),
        pl::ExprKind::Array(v) => ExprKind::Array(restrict_exprs(v)),
        pl::ExprKind::Range(v) => ExprKind::Range(v.map(restrict_expr_box)),
        pl::ExprKind::Binary(v) => ExprKind::Binary(BinaryExpr {
            left: restrict_expr_box(v.left),
            op: v.op,
            right: restrict_expr_box(v.right),
        }),
        pl::ExprKind::Unary(v) => ExprKind::Unary(prql_ast::expr::UnaryExpr {
            op: v.op,
            expr: restrict_expr_box(v.expr),
        }),
        pl::ExprKind::FuncCall(v) => ExprKind::FuncCall(prql_ast::expr::FuncCall {
            name: restrict_expr_box(v.name),
            args: restrict_exprs(v.args),
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| (k, restrict_expr(v)))
                .collect(),
        }),
        pl::ExprKind::Func(v) => ExprKind::Func(
            prql_ast::expr::Func {
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
                .map(|case| prql_ast::expr::SwitchCase {
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

fn restrict_func_params(value: Vec<pl::FuncParam>) -> Vec<prql_ast::expr::FuncParam> {
    value.into_iter().map(restrict_func_param).collect()
}

fn restrict_func_param(value: pl::FuncParam) -> prql_ast::expr::FuncParam {
    prql_ast::expr::FuncParam {
        name: value.name,
        ty: value.ty.map(restrict_ty_or_expr).map(Box::new),
        default_value: value.default_value.map(restrict_expr_box),
    }
}

fn restrict_ty_or_expr(value: pl::TyOrExpr) -> prql_ast::expr::Expr {
    match value {
        pl::TyOrExpr::Ty(ty) => restrict_ty(ty),
        pl::TyOrExpr::Expr(expr) => restrict_expr(*expr),
    }
}

fn restrict_ty(value: pl::Ty) -> prql_ast::expr::Expr {
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
    };
    Expr::new(expr_kind)
}
