use super::{Expr, ExprKind, FuncCall};
use crate::ast::Ident;

pub fn maybe_binop(left: Option<Expr>, op_name: &[&str], right: Option<Expr>) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => Some(new_binop(left, op_name, right)),
        (left, right) => left.or(right),
    }
}

pub fn new_binop(left: Expr, op_name: &[&str], right: Expr) -> Expr {
    Expr::new(ExprKind::FuncCall(FuncCall {
        name: Box::new(Expr::new(Ident::from_path(op_name.to_vec()))),
        args: vec![left, right],
        named_args: Default::default(),
    }))
}
