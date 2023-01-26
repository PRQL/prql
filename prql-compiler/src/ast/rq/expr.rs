use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::super::pl::{BinOp, InterpolateItem, Literal, SwitchCase};
use super::CId;
use crate::error::Span;

/// Analogous to [crate::ast::pl::Expr], but with less kinds.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Option<Span>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum ExprKind {
    ColumnRef(CId),
    Literal(Literal),

    // TODO: convert this into built-in function
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },

    // TODO: convert this into built-in function
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },

    SString(Vec<InterpolateItem<Expr>>),

    // TODO: convert this into built-in function
    FString(Vec<InterpolateItem<Expr>>),
    Switch(Vec<SwitchCase<Expr>>),

    BuiltInFunction {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Not,
}

impl From<ExprKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(_kind: ExprKind) -> Self {
        panic!("Failed to convert ir:ExprKind")
        // anyhow!("Failed to convert ir:ExprKind")
    }
}
