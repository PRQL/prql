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
                if comparable_literals(left, right) {
                    return Expr::new(Literal::Boolean(left == right));
                }
            }
        }
        "std.ne" => {
            if let (ExprKind::Literal(left), ExprKind::Literal(right)) =
                (&args[0].kind, &args[1].kind)
            {
                if comparable_literals(left, right) {
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

/// Whether two literals can be compared at compile time by their values.
///
/// Literals of different types aren't compared. Times, timestamps and intervals
/// aren't either: they are kept as written, and the database treats some
/// differently written values as equal (`@10:00` and `@10:00:00`, `24hours` and
/// `1days`).
fn comparable_literals(left: &Literal, right: &Literal) -> bool {
    left.as_ref() == right.as_ref()
        && !matches!(
            left,
            Literal::Time(_) | Literal::Timestamp(_) | Literal::ValueAndUnit(_)
        )
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

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    #[test]
    fn test_eq_temporal_literals_not_folded() {
        // Times, timestamps and intervals can be written in several ways that the
        // database treats as equal, so comparing their source text would be wrong.
        let query = "
        from t
        derive {
          a = @2020-01-01T10:00 == @2020-01-01T10:00:00,
          b = 24hours == 1days,
          c = @10:00 != @10:00:00,
          d = 1 == 1,
          e = 'x' != 'x',
        }
        ";
        assert_snapshot!(crate::tests::compile(query).unwrap(), @"
        SELECT
          *,
          TIMESTAMP '2020-01-01T10:00' = TIMESTAMP '2020-01-01T10:00:00' AS a,
          INTERVAL 24 HOUR = INTERVAL 1 DAY AS b,
          TIME '10:00' <> TIME '10:00:00' AS c,
          true AS d,
          false AS e
        FROM
          t
        ");
    }
}
