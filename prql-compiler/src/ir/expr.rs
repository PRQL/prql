use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::{CId, TId};
use crate::{
    ast::{BinOp, InterpolateItem, Literal, Range},
    error::Span,
};

/// Analogous to [crate::ast::Expr], but with stricter.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Option<Span>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum ExprKind {
    ColumnRef(CId),
    ExternRef {
        variable: String,
        table: Option<TId>,
    },
    Literal(Literal),
    Range(Range<Box<Expr>>),
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    SString(Vec<InterpolateItem<Expr>>),
    FString(Vec<InterpolateItem<Expr>>),
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
