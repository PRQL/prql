use crate::ast::{Literal, Span};
use serde::{Deserialize, Serialize};

/// A subset of PL expressions that are constant.
#[derive(Serialize, Deserialize)]
pub struct ConstExpr {
    pub kind: ConstExprKind,

    pub span: Option<Span>,
}

/// A subset of PL expressions that are constant.
#[derive(Serialize, Deserialize)]
pub struct ConstTupleField {
    pub name: Option<String>,

    pub value: ConstExpr,
}



#[derive(Serialize, Deserialize)]
pub enum ConstExprKind {
    Literal(Literal),
    Tuple(Vec<ConstTupleField>),
    Array(Vec<ConstExpr>),
}
