//! Static analysis - compile time expression evaluation

use crate::ast::pl::{BinOp, Expr, ExprKind, Literal, UnOp};

pub fn static_analysis(expr: Expr) -> Expr {
    // let is_const = match &expr.kind {
    //     ExprKind::Ident(_) => Some(false),
    //     ExprKind::Literal(_) => Some(true),

    //     ExprKind::Pipeline(pipeline) => pipeline.exprs.iter().map(|e| &e.is_const).all_true(),
    //     ExprKind::List(items) => items.iter().map(|e| &e.is_const).all_true(),
    //     ExprKind::Range(r) => {
    //         let exprs = r.start.iter().chain(r.end.iter());

    //         exprs.map(|e| &e.is_const).all_true()
    //     }

    //     ExprKind::Binary { left, op: _, right } => {
    //         let exprs = [left, right];

    //         exprs.iter().map(|e| &e.is_const).all_true()
    //     }
    //     ExprKind::Unary { op: _, expr } => expr.is_const.clone(),

    //     _ => None,
    // };

    let kind = eval(expr.kind);

    Expr { kind, ..expr }
}

fn eval(kind: ExprKind) -> ExprKind {
    match kind {
        ExprKind::Unary { op, expr } => {
            let res = if let ExprKind::Literal(lit) = &expr.kind {
                match (op, lit) {
                    (UnOp::Not, Literal::Boolean(val)) => Some(Literal::Boolean(!val)),
                    (UnOp::Neg, Literal::Integer(val)) => Some(Literal::Integer(-val)),
                    (UnOp::Neg, Literal::Float(val)) => Some(Literal::Float(-val)),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(lit) = res {
                ExprKind::Literal(lit)
            } else {
                ExprKind::Unary { op, expr }
            }
        }
        ExprKind::Binary { left, op, right } => {
            let res = if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                (&left.kind, &right.kind)
            {
                match (op, left, right) {
                    (BinOp::Mul, Literal::Integer(left), Literal::Integer(right)) => {
                        Some(Literal::Integer(left * right))
                    }
                    (BinOp::Mul, Literal::Float(left), Literal::Float(right)) => {
                        Some(Literal::Float(left * right))
                    }
                    (BinOp::Div, Literal::Integer(left), Literal::Integer(right)) => {
                        Some(Literal::Integer(left / right))
                    }
                    (BinOp::Div, Literal::Float(left), Literal::Float(right)) => {
                        Some(Literal::Float(left / right))
                    }
                    (BinOp::Mod, Literal::Integer(left), Literal::Integer(right)) => {
                        Some(Literal::Integer(left % right))
                    }
                    (BinOp::Mod, Literal::Float(left), Literal::Float(right)) => {
                        Some(Literal::Float(left % right))
                    }

                    (BinOp::Add, Literal::Integer(left), Literal::Integer(right)) => {
                        Some(Literal::Integer(left + right))
                    }
                    (BinOp::Add, Literal::Float(left), Literal::Float(right)) => {
                        Some(Literal::Float(left + right))
                    }
                    (BinOp::Sub, Literal::Integer(left), Literal::Integer(right)) => {
                        Some(Literal::Integer(left - right))
                    }
                    (BinOp::Sub, Literal::Float(left), Literal::Float(right)) => {
                        Some(Literal::Float(left - right))
                    }

                    (BinOp::Eq, left, right) => {
                        // don't eval comparisons between different types of literals
                        if left.as_ref() != right.as_ref() {
                            None
                        } else {
                            Some(Literal::Boolean(left == right))
                        }
                    }
                    (BinOp::Ne, left, right) => {
                        // don't eval comparisons between different types of literals
                        if left.as_ref() != right.as_ref() {
                            None
                        } else {
                            Some(Literal::Boolean(left == right))
                        }
                    }

                    (BinOp::Gt, _, _) => None,
                    (BinOp::Lt, _, _) => None,
                    (BinOp::Gte, _, _) => None,
                    (BinOp::Lte, _, _) => None,

                    (BinOp::And, Literal::Boolean(left), Literal::Boolean(right)) => {
                        Some(Literal::Boolean(*left && *right))
                    }
                    (BinOp::Or, Literal::Boolean(left), Literal::Boolean(right)) => {
                        Some(Literal::Boolean(*left || *right))
                    }

                    _ => None,
                }
            } else {
                None
            };

            if let Some(lit) = res {
                ExprKind::Literal(lit)
            } else if let (BinOp::Coalesce, ExprKind::Literal(Literal::Null)) = (op, &left.kind) {
                right.kind
            } else {
                ExprKind::Binary { left, op, right }
            }
        }

        ExprKind::Switch(items) => {
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
                return ExprKind::Literal(Literal::Null);
            }

            if res.len() == 1 {
                let is_true = matches!(
                    res[0].condition.kind,
                    ExprKind::Literal(Literal::Boolean(true))
                );
                if is_true {
                    return res.remove(0).value.kind;
                }
            }

            ExprKind::Switch(res)
        }

        k => k,
    }
}
