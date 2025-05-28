use enum_as_inner::EnumAsInner;
use prqlc_parser::generic;
use prqlc_parser::lexer::lr::Literal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::CId;
use crate::Span;

/// Analogous to [crate::ir::pl::Expr], but with fewer kinds.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Option<Span>,
}

pub(super) type Range = generic::Range<Expr>;
pub(super) type InterpolateItem = generic::InterpolateItem<Expr>;
pub(super) type SwitchCase = generic::SwitchCase<Expr>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner, JsonSchema)]
pub enum ExprKind {
    ColumnRef(CId),
    // https://github.com/dtolnay/serde-yaml/issues/363
    // We should repeat this if we encounter any other nested enums.
    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map"),
        schemars(with = "Literal")
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
    SqlArray(Vec<Expr>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum UnOp {
    Neg,
    Not,
}
