use super::{Expr, ExprKind};
use crate::ast::pl::BinOp;

pub fn new_binop(left: Option<Expr>, op: BinOp, right: Option<Expr>) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let left = Box::new(left);
            let right = Box::new(right);
            Some(Expr {
                kind: ExprKind::Binary { left, op, right },
                span: None,
            })
        }
        (left, right) => left.or(right),
    }
}
