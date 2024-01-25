use prqlc_ast::{Literal, Span};

pub struct ConstExpr {
    pub kind: ConstExprKind,

    pub span: Option<Span>,
}

pub enum ConstExprKind {
    Literal(Literal),
    Tuple(Vec<ConstExpr>),
    Array(Vec<ConstExpr>),
}
