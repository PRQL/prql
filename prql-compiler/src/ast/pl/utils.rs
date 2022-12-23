use super::{BinOp, Expr, ExprKind};

pub fn new_binop(left: Option<Expr>, op: BinOp, right: Option<Expr>) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let left = Box::new(left);
            let right = Box::new(right);
            Some(Expr::from(ExprKind::Binary { left, op, right }))
        }
        (left, right) => left.or(right),
    }
}
