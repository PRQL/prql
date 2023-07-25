use super::Expr;
use super::ExprKind;

pub fn new_binop(left: Expr, operator_name: &str, right: Expr) -> Expr {
    Expr {
        kind: ExprKind::Operator {
            name: operator_name.to_string(),
            args: vec![left, right],
        },
        span: None,
    }
}

pub fn maybe_binop(left: Option<Expr>, operator_name: &str, right: Option<Expr>) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => Some(Expr {
            kind: ExprKind::Operator {
                name: operator_name.to_string(),
                args: vec![left, right],
            },
            span: None,
        }),
        (left, right) => left.or(right),
    }
}
