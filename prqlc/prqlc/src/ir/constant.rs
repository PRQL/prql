use prqlc_ast::{Literal, Span};
use serde::{Deserialize, Serialize};

/// A subset of PL expressions that are constant.
#[derive(Serialize, Deserialize)]
pub struct ConstExpr {
    pub kind: ConstExprKind,

    pub span: Option<Span>,
}

#[derive(Serialize, Deserialize)]
pub enum ConstExprKind {
    Literal(Literal),
    Tuple(Vec<ConstExpr>),
    Array(Vec<ConstExpr>),
}
