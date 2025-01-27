//! Static analysis - compile time expression evaluation

use crate::ir::pl::{Expr, ExprKind, Literal};
use crate::Result;

impl super::Resolver<'_> {
    /// Tries to simplify this expression (and not child expressions) to a constant.
    pub fn maybe_static_eval(&mut self, expr: Expr) -> Result<Expr> {
        Ok(match &expr.kind {
            ExprKind::RqOperator { .. } => {
                let id = expr.id;
                let span = expr.span;
                let expr = static_eval_rq_operator(expr);
                Expr { id, span, ..expr }
            }

            ExprKind::Case(_) => static_eval_case(expr),

            _ => expr,
        })
    }
}

fn static_eval_rq_operator(mut expr: Expr) -> Expr {
    let (name, mut args) = expr.kind.into_rq_operator().unwrap();
    match name.as_str() {
        "std.not" => {
            if let ExprKind::Literal(Literal::Boolean(val)) = &args[0].kind {
                return Expr::new(Literal::Boolean(!val));
            }
        }
        "std.neg" => match &args[0].kind {
            ExprKind::Literal(Literal::Integer(val)) => return Expr::new(Literal::Integer(-val)),
            ExprKind::Literal(Literal::Float(val)) => return Expr::new(Literal::Float(-val)),
            _ => (),
        },

        "std.eq" => {
            if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                (&args[0].kind, &args[1].kind)
            {
                // don't eval comparisons between different types of literals
                if left.as_ref() == right.as_ref() {
                    return Expr::new(Literal::Boolean(left == right));
                }
            }
        }
        "std.ne" => {
            if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                (&args[0].kind, &args[1].kind)
            {
                // don't eval comparisons between different types of literals
                if left.as_ref() == right.as_ref() {
                    return Expr::new(Literal::Boolean(left != right));
                }
            }
        }
        "std.and" => {
            if let (
                ExprKind::Literal(Literal::Boolean(left)),
                ExprKind::Literal(Literal::Boolean(right)),
            ) = (&args[0].kind, &args[1].kind)
            {
                return Expr::new(Literal::Boolean(*left && *right));
            }
        }
        "std.or" => {
            if let (
                ExprKind::Literal(Literal::Boolean(left)),
                ExprKind::Literal(Literal::Boolean(right)),
            ) = (&args[0].kind, &args[1].kind)
            {
                return Expr::new(Literal::Boolean(*left || *right));
            }
        }
        "std.coalesce" => {
            if let ExprKind::Literal(Literal::Null) = &args[0].kind {
                return args.remove(1);
            }
        }

        _ => {}
    };
    expr.kind = ExprKind::RqOperator { name, args };
    expr
}

fn static_eval_case(mut expr: Expr) -> Expr {
    let items = expr.kind.into_case().unwrap();
    let mut res = Vec::with_capacity(items.len());
    for item in items {
        if let ExprKind::Literal(Literal::Boolean(condition)) = item.condition.kind {
            if condition {
                res.push(item);
                break;
            } else {
                // this case can be removed
                continue;
            }
        } else {
            res.push(item);
        }
    }
    if res.is_empty() {
        return Expr::new(Literal::Null);
    }

    if res.len() == 1 {
        let is_true = matches!(
            res[0].condition.kind,
            ExprKind::Literal(Literal::Boolean(true))
        );
        if is_true {
            return *res.remove(0).value;
        }
    }

    expr.kind = ExprKind::Case(res);
    expr
}
