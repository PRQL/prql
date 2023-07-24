use std::collections::HashMap;

use prql_ast::expr::{Expr, ExprKind};
use prql_ast::stmt::{Annotation, Stmt, StmtKind, VarDefKind};

use crate::ir::pl;

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
            stmts: v.stmts.into_iter().map(expand_stmt).collect(),
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
