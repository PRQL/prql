use crate::ast::rq::{Expr, ExprKind};

pub(super) fn new_binop(left: Expr, func: super::std::FunctionDecl<2>, right: Expr) -> Expr {
    Expr {
        kind: ExprKind::BuiltInFunction {
            name: func.name.to_string(),
            args: vec![left, right],
        },
        span: None,
    }
}

pub(super) fn maybe_binop(
    left: Option<Expr>,
    func: super::std::FunctionDecl<2>,
    right: Option<Expr>,
) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => Some(Expr {
            kind: ExprKind::BuiltInFunction {
                name: func.name.to_string(),
                args: vec![left, right],
            },
            span: None,
        }),
        (left, right) => left.or(right),
    }
}
