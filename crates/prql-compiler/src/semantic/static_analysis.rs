//! Static analysis - compile time expression evaluation

use crate::ir::pl::{Expr, ExprKind, Literal};

pub fn static_analysis(mut expr: Expr) -> Expr {
    expr.kind = eval(expr.kind);

    if expr.kind.is_literal() {
        expr.ty = None;
    }
    expr
}

fn eval(kind: ExprKind) -> ExprKind {
    match kind {
        ExprKind::RqOperator { name, mut args } => {
            match name.as_str() {
                "std.not" => {
                    if let ExprKind::Literal(Literal::Boolean(val)) = &args[0].kind {
                        return ExprKind::Literal(Literal::Boolean(!val));
                    }
                }
                "std.neg" => match &args[0].kind {
                    ExprKind::Literal(Literal::Integer(val)) => {
                        return ExprKind::Literal(Literal::Integer(-val))
                    }
                    ExprKind::Literal(Literal::Float(val)) => {
                        return ExprKind::Literal(Literal::Float(-val))
                    }
                    _ => (),
                },

                "std.eq" => {
                    if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                        (&args[0].kind, &args[1].kind)
                    {
                        // don't eval comparisons between different types of literals
                        if left.as_ref() == right.as_ref() {
                            return ExprKind::Literal(Literal::Boolean(left == right));
                        }
                    }
                }
                "std.ne" => {
                    if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                        (&args[0].kind, &args[1].kind)
                    {
                        // don't eval comparisons between different types of literals
                        if left.as_ref() == right.as_ref() {
                            return ExprKind::Literal(Literal::Boolean(left != right));
                        }
                    }
                }
                "std.and" => {
                    if let (
                        ExprKind::Literal(Literal::Boolean(left)),
                        ExprKind::Literal(Literal::Boolean(right)),
                    ) = (&args[0].kind, &args[1].kind)
                    {
                        return ExprKind::Literal(Literal::Boolean(*left && *right));
                    }
                }
                "std.or" => {
                    if let (
                        ExprKind::Literal(Literal::Boolean(left)),
                        ExprKind::Literal(Literal::Boolean(right)),
                    ) = (&args[0].kind, &args[1].kind)
                    {
                        return ExprKind::Literal(Literal::Boolean(*left || *right));
                    }
                }
                "std.coalesce" => {
                    if let ExprKind::Literal(Literal::Null) = &args[0].kind {
                        return args.remove(1).kind;
                    }
                }

                _ => {}
            };
            ExprKind::RqOperator { name, args }
        }

        ExprKind::Case(items) => {
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

            ExprKind::Case(res)
        }

        k => k,
    }
}
