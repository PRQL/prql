use crate::ast::expr::Ident;

use super::{Expr, ExprKind, FuncCall};

pub fn new_binop(left: Expr, op_name: &[&str], right: Expr) -> Expr {
    Expr::new(ExprKind::FuncCall(FuncCall {
        name: Box::new(Expr::new(Ident::from_path(op_name.to_vec()))),
        args: vec![left, right],
        named_args: Default::default(),
    }))
}
