use std::collections::HashMap;

use prql_ast::expr::ExprKind;
use prql_ast::stmt::{StmtKind, VarDefKind};

use crate::ir::pl;

fn map_vec_into<I: Into<O>, O>(exprs: Vec<I>) -> Vec<O> {
    exprs.into_iter().map(Into::into).collect::<Vec<_>>()
}

#[allow(clippy::boxed_local)]
fn map_box_into<I: Into<O>, O>(expr: Box<I>) -> Box<O> {
    Box::new((*expr).into())
}

impl From<prql_ast::expr::Expr> for pl::Expr {
    fn from(value: prql_ast::expr::Expr) -> Self {
        Self {
            kind: value.kind.into(),
            span: value.span,
            alias: value.alias,
            id: None,
            target_id: None,
            target_ids: Vec::new(),
            ty: None,
            lineage: None,
            needs_window: false,
            flatten: false,
        }
    }
}

impl From<ExprKind> for pl::ExprKind {
    fn from(value: ExprKind) -> Self {
        match value {
            ExprKind::Ident(v) => Self::Ident(v),
            ExprKind::Literal(v) => Self::Literal(v),
            ExprKind::Pipeline(v) => Self::Pipeline(pl::Pipeline {
                exprs: map_vec_into(v.exprs),
            }),
            ExprKind::Tuple(v) => Self::Tuple(map_vec_into(v)),
            ExprKind::Array(v) => Self::Array(map_vec_into(v)),
            ExprKind::Range(v) => Self::Range(v.map(|expr| pl::Expr::from(*expr).into())),
            ExprKind::Binary(v) => Self::Binary(pl::BinaryExpr {
                left: map_box_into(v.left),
                op: v.op,
                right: map_box_into(v.right),
            }),
            ExprKind::Unary(v) => Self::Unary(pl::UnaryExpr {
                op: v.op,
                expr: map_box_into(v.expr),
            }),
            ExprKind::FuncCall(v) => Self::FuncCall(pl::FuncCall {
                name: map_box_into(v.name),
                args: map_vec_into(v.args),
                named_args: v
                    .named_args
                    .into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect(),
            }),
            ExprKind::Func(v) => Self::Func(
                pl::Func {
                    return_ty: v.return_ty.map(Into::into),
                    body: map_box_into(v.body),
                    params: map_vec_into(v.params),
                    named_params: map_vec_into(v.named_params),
                    name_hint: None,
                    args: Vec::new(),
                    env: HashMap::new(),
                }
                .into(),
            ),
            ExprKind::SString(v) => {
                Self::SString(v.into_iter().map(|v| v.map(Into::into)).collect())
            }
            ExprKind::FString(v) => {
                Self::FString(v.into_iter().map(|v| v.map(Into::into)).collect())
            }
            ExprKind::Case(v) => Self::Case(
                v.into_iter()
                    .map(|case| pl::SwitchCase {
                        condition: map_box_into(case.condition),
                        value: map_box_into(case.value),
                    })
                    .collect(),
            ),
            ExprKind::Param(v) => Self::Param(v),
            ExprKind::Internal(v) => Self::Internal(v),
        }
    }
}

impl From<prql_ast::expr::FuncParam> for pl::FuncParam {
    fn from(value: prql_ast::expr::FuncParam) -> Self {
        Self {
            name: value.name,
            ty: value.ty.map(Into::into),
            default_value: value.default_value.map(map_box_into),
        }
    }
}

impl From<Box<prql_ast::expr::Expr>> for pl::TyOrExpr {
    fn from(value: Box<prql_ast::expr::Expr>) -> Self {
        Self::Expr(Box::new((*value).into()))
    }
}

impl From<prql_ast::stmt::Stmt> for pl::Stmt {
    fn from(value: prql_ast::stmt::Stmt) -> Self {
        Self {
            id: None,
            kind: value.kind.into(),
            span: value.span,
            annotations: value.annotations.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<StmtKind> for pl::StmtKind {
    fn from(value: prql_ast::stmt::StmtKind) -> Self {
        match value {
            StmtKind::QueryDef(v) => Self::QueryDef(v),
            StmtKind::Main(v) => Self::VarDef(pl::VarDef {
                name: None,
                value: map_box_into(v),
                ty_expr: None,
                kind: pl::VarDefKind::Main,
            }),
            StmtKind::VarDef(v) => Self::VarDef(pl::VarDef {
                name: Some(v.name),
                value: map_box_into(v.value),
                ty_expr: v.ty_expr.map(map_box_into),
                kind: v.kind.into(),
            }),
            StmtKind::TypeDef(v) => Self::TypeDef(pl::TypeDef {
                name: v.name,
                value: v.value.map(map_box_into),
            }),
            StmtKind::ModuleDef(v) => Self::ModuleDef(pl::ModuleDef {
                name: v.name,
                stmts: map_vec_into(v.stmts),
            }),
        }
    }
}

impl From<VarDefKind> for pl::VarDefKind {
    fn from(value: VarDefKind) -> Self {
        match value {
            VarDefKind::Let => Self::Let,
            VarDefKind::Into => Self::Into,
        }
    }
}

impl From<prql_ast::stmt::Annotation> for pl::Annotation {
    fn from(value: prql_ast::stmt::Annotation) -> Self {
        Self {
            expr: map_box_into(value.expr),
        }
    }
}
