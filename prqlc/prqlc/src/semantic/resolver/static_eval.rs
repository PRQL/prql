//! Static analysis - compile time expression evaluation

use itertools::Itertools;

use crate::ir::constant::{ConstExpr, ConstExprKind};
use crate::ir::pl::{Expr, ExprKind, Literal, PlFold};
use crate::{Error, Result, WithErrorInfo};

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

    /// Simplify an expression to a constant, recursively.
    pub fn static_eval_to_constant(&mut self, expr: Expr) -> Result<ConstExpr> {
        StaticEvaluator::run(expr, self)
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

struct StaticEvaluator<'a, 'b> {
    resolver: &'a mut super::Resolver<'b>,
}

impl<'a, 'b> StaticEvaluator<'a, 'b> {
    fn run(expr: Expr, resolver: &'a mut super::Resolver<'b>) -> Result<ConstExpr> {
        let expr = StaticEvaluator { resolver }.fold_expr(expr)?;
        restrict_to_const(expr)
    }
}

impl PlFold for StaticEvaluator<'_, '_> {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        expr.kind = self.fold_expr_kind(expr.kind)?;
        self.resolver.maybe_static_eval(expr)
    }
}

fn restrict_to_const(expr: Expr) -> Result<ConstExpr, Error> {
    let kind = match expr.kind {
        ExprKind::Literal(lit) => ConstExprKind::Literal(lit),
        ExprKind::Tuple(fields) => {
            ConstExprKind::Tuple(fields.into_iter().map(restrict_to_const).try_collect()?)
        }
        ExprKind::Array(items) => {
            ConstExprKind::Array(items.into_iter().map(restrict_to_const).try_collect()?)
        }
        _ => {
            // everything else is not a constant
            return Err(Error::new_simple("not a constant").with_span(expr.span));
        }
    };
    Ok(ConstExpr {
        span: expr.span,
        kind,
    })
}
