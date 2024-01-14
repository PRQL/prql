use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::super::pl::Literal;
use super::CId;
use crate::Span;

/// Analogous to [crate::ir::pl::Expr], but with fewer kinds.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Option<Span>,
}

pub(super) type Range = prqlc_ast::expr::generic::Range<Expr>;
pub(super) type InterpolateItem = prqlc_ast::expr::generic::InterpolateItem<Expr>;
pub(super) type SwitchCase = prqlc_ast::expr::generic::SwitchCase<Expr>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum ExprKind {
    ColumnRef(CId),
    // https://github.com/dtolnay/serde-yaml/issues/363
    // We should repeat this if we encounter any other nested enums.
    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map")
    )]
    Literal(Literal),

    SString(Vec<InterpolateItem>),

    Case(Vec<SwitchCase>),

    Operator {
        name: String,
        args: Vec<Expr>,
    },

    /// Placeholder for expressions provided after compilation.
    Param(String),

    Array(Vec<Expr>),
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
