use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::super::pl::{InterpolateItem, Literal, SwitchCase};
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
    // https://github.com/dtolnay/serde-yaml/issues/363
    // We should repeat this if we encounter any other nested enums.
    #[serde(with = "serde_yaml::with::singleton_map")]
    Literal(Literal),

    SString(Vec<InterpolateItem<Expr>>),

    Case(Vec<SwitchCase<Expr>>),

    Operator {
        name: String,
        args: Vec<Expr>,
    },

    /// Placeholder for expressions provided after compilation.
    Param(String),
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
