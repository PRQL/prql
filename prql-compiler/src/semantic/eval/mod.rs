use anyhow::Result;

use crate::{
    ast::pl::{fold::AstFold, Expr, ExprKind, Func, FuncCall, Literal},
    error::WithErrorInfo,
    Error, Span,
};

use super::resolver::{binary_to_func_call, unary_to_func_call};

pub fn eval(expr: Expr) -> Result<Expr> {
    Evaluator {}.fold_expr(expr)
}

/// Converts an expression to a value
///
/// Serves as a working draft of PRQL semantics definition.
struct Evaluator {}

impl AstFold for Evaluator {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        let mut expr = super::static_analysis::static_analysis(expr);

        expr.kind = match expr.kind {
            // these are values already
            ExprKind::Literal(l) => ExprKind::Literal(l),

            // these are values, iff their contents are values too
            ExprKind::Array(_) | ExprKind::Tuple(_) | ExprKind::Range(_) => {
                self.fold_expr_kind(expr.kind)?
            }

            // functions are values
            ExprKind::Func(f) => ExprKind::Func(f),

            // convert to function calls and then evaluate to a value
            ExprKind::Binary(binary) => {
                let func_call = binary_to_func_call(binary, expr.span);
                self.fold_expr(func_call)?.kind
            }
            ExprKind::Unary(unary) => {
                let func_call = unary_to_func_call(unary, expr.span);
                self.fold_expr(func_call)?.kind
            }

            // ident are not values
            ExprKind::Ident(ident) => {
                // this is very crude, but for simple cases, it's enough
                if ident.path.get(0).map(|x| x == "std").unwrap_or_default() {
                    ExprKind::Func(Box::new(Func {
                        name_hint: Some(ident),

                        // these don't matter
                        return_ty: Default::default(),
                        body: Box::new(Expr::null()),
                        params: Default::default(),
                        named_params: Default::default(),
                        args: Default::default(),
                        env: Default::default(),
                    }))
                } else {
                    todo!()
                }
            }

            // the beef happens here
            ExprKind::FuncCall(func_call) => {
                let func = self.fold_expr(*func_call.name)?;
                let func = *func.try_cast(|x| x.into_func(), Some("func call"), "function")?;
                let func_name = func.name_hint.unwrap().to_string();

                let args = self.fold_exprs(func_call.args)?;

                eval_function(&func_name, args, expr.span)?
            }
            ExprKind::Pipeline(mut pipeline) => {
                let mut res = self.fold_expr(pipeline.exprs.remove(0))?;
                for expr in pipeline.exprs {
                    let func_call =
                        Expr::from(ExprKind::FuncCall(FuncCall::new_simple(expr, vec![res])));

                    res = self.fold_expr(func_call)?;
                }

                return Ok(res);
            }

            ExprKind::All { .. }
            | ExprKind::TransformCall(_)
            | ExprKind::SString(_)
            | ExprKind::FString(_)
            | ExprKind::Case(_)
            | ExprKind::RqOperator { .. }
            | ExprKind::Type(_)
            | ExprKind::Param(_)
            | ExprKind::Internal(_) => {
                return Err(Error::new_simple("not a value").with_span(expr.span).into())
            }
        };
        Ok(expr)
    }
}

fn eval_function(name: &str, args: Vec<Expr>, span: Option<Span>) -> Result<ExprKind> {
    use Literal::*;

    Ok(match name {
        "std.add" => {
            let [l, r]: [_; 2] = args.try_into().unwrap();

            let l = l.kind.into_literal().unwrap();
            let r = r.kind.into_literal().unwrap();

            let res = match (l, r) {
                (Integer(l), Integer(r)) => (l + r) as f64,
                (Float(l), Integer(r)) => l + (r as f64),
                (Integer(l), Float(r)) => (l as f64) + r,
                (Float(l), Float(r)) => l + r,

                _ => return Err(Error::new_simple("bad arg types").with_span(span).into()),
            };

            ExprKind::Literal(Float(res))
        }

        "std.floor" => {
            let [x]: [_; 1] = args.try_into().unwrap();

            let res = match x.kind {
                ExprKind::Literal(Integer(i)) => i,
                ExprKind::Literal(Float(f)) => f.floor() as i64,
                _ => return Err(Error::new_simple("bad arg types").with_span(x.span).into()),
            };

            ExprKind::Literal(Integer(res))
        }

        "std.neg" => {
            let [x]: [_; 1] = args.try_into().unwrap();

            match x.kind {
                ExprKind::Literal(Integer(i)) => ExprKind::Literal(Integer(-i)),
                ExprKind::Literal(Float(f)) => ExprKind::Literal(Float(-f)),
                _ => return Err(Error::new_simple("bad arg types").with_span(x.span).into()),
            }
        }

        _ => {
            return Err(Error::new_simple(format!("unknown function {name}"))
                .with_span(span)
                .into())
        }
    })
}

#[cfg(test)]
mod test {

    use insta::assert_display_snapshot;
    use itertools::Itertools;

    use super::*;

    fn eval(source: &str) -> Result<String> {
        let stmts = crate::prql_to_pl(source)?.into_iter().exactly_one()?;
        let expr = stmts.kind.into_var_def()?.value;

        let value = super::eval(*expr)?;

        Ok(value.to_string())
    }

    #[test]
    fn basic() {
        assert_display_snapshot!(eval(r"
            [std.floor (3.5 + 2.9) + 3, 3]
        ").unwrap(),
            @"[9, 3]"
        );
    }

    #[test]
    fn tuples() {
        assert_display_snapshot!(eval(r"
              {{a_a = 4, a_b = false}, b = 2.1 + 3.6, c = [false, true, false]}
        ").unwrap(),
            @"{{a_a = 4, a_b = false}, b = 5.7, c = [false, true, false]}"
        );
    }

    #[test]
    fn pipelines() {
        assert_display_snapshot!(eval(r"
            (4.5 | std.floor | std.neg)
        ").unwrap(),
            @"-4"
        );
    }
}
