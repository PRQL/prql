use std::iter::zip;

use anyhow::Result;
use itertools::Itertools;

use crate::ast::pl::{fold::AstFold, Expr, ExprKind, Func, FuncCall, FuncParam, Ident, Literal};
use crate::error::{Error, Span, WithErrorInfo};

use super::resolver::{binary_to_func_call, unary_to_func_call};

pub fn eval(expr: Expr) -> Result<Expr> {
    Evaluator::new().fold_expr(expr)
}

/// Converts an expression to a value
///
/// Serves as a working draft of PRQL semantics definition.
struct Evaluator {
    relation: Option<Expr>,
}

impl Evaluator {
    fn new() -> Self {
        Evaluator { relation: None }
    }
}

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
                // here we'd have to implement the whole name resolution, but for now,
                // let's do something simple

                // this is very crude, but for simple cases, it's enough
                let mut ident = ident;
                let mut base = self.relation.clone();
                loop {
                    let (first, remaining) = ident.pop_front();
                    let res = lookup(base.as_ref(), &first).with_span(expr.span)?;

                    if let Some(remaining) = remaining {
                        ident = remaining;
                        base = Some(res);
                    } else {
                        return Ok(res);
                    }
                }
            }

            // the beef happens here
            ExprKind::FuncCall(func_call) => {
                let func = self.fold_expr(*func_call.name)?;
                let mut func = func.try_cast(|x| x.into_func(), Some("func call"), "function")?;

                func.args.extend(func_call.args);

                if func.args.len() < func.params.len() {
                    ExprKind::Func(func)
                } else {
                    self.eval_function(*func, expr.span)?
                }
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

fn lookup(base: Option<&Expr>, name: &str) -> Result<Expr> {
    if let Some(base) = base {
        if let ExprKind::Tuple(items) = &base.kind {
            if let Some(item) = items.iter().find(|i| i.alias.as_deref() == Some(name)) {
                return Ok(item.clone());
            }
        }
    }
    if name == "std" {
        return Ok(std_module());
    }

    Err(Error::new_simple(format!("cannot find `{}` in {:?}", name, base)).into())
}

impl Evaluator {
    fn eval_function(&mut self, func: Func, span: Option<Span>) -> Result<ExprKind> {
        let func_name = func.name_hint.unwrap().to_string();

        // eval args
        let closure = (func.params.iter()).find_position(|x| x.name == "closure");

        let args = if let Some((closure_position, _)) = closure {
            let mut args = Vec::new();

            for (pos, arg) in func.args.into_iter().enumerate() {
                if pos == closure_position {
                    let closure = Expr::from(ExprKind::Func(Box::new(Func {
                        name_hint: None,

                        params: Default::default(),
                        body: Box::new(arg),
                        return_ty: Default::default(),
                        named_params: Default::default(),
                        args: Default::default(),
                        env: Default::default(),
                    })));

                    args.push(closure);
                } else {
                    args.push(self.fold_expr(arg)?);
                }
            }
            args
        } else {
            self.fold_exprs(func.args)?
        };

        // eval body
        use Literal::*;
        Ok(match func_name.as_str() {
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

            "std.select" => {
                let [tuple_closure, relation]: [_; 2] = args.try_into().unwrap();

                self.eval_for_each_row(relation, tuple_closure)?.kind
            }

            "std.derive" => {
                let [tuple_closure, relation]: [_; 2] = args.try_into().unwrap();

                let new = self.eval_for_each_row(relation.clone(), tuple_closure)?;

                zip_relations(relation, new)
            }

            "std.filter" => {
                let [condition_closure, relation]: [_; 2] = args.try_into().unwrap();

                let condition = self.eval_for_each_row(relation.clone(), condition_closure)?;

                let condition = condition.kind.into_array().unwrap();
                let relation = relation.kind.into_array().unwrap();

                let mut res = Vec::new();
                for (cond, tuple) in zip(condition, relation) {
                    let f = cond.kind.into_literal().unwrap().into_boolean().unwrap();

                    if f {
                        res.push(tuple);
                    }
                }

                ExprKind::Array(res)
            }

            "std.aggregate" => {
                let [tuple_closure, relation]: [_; 2] = args.try_into().unwrap();

                let tuple = self.eval_for_all_rows(relation, tuple_closure)?;

                ExprKind::Array(vec![tuple])
            }

            "std.sum" => {
                let [array]: [_; 1] = args.try_into().unwrap();

                let mut sum = 0.0;
                for item in array.kind.into_array().unwrap() {
                    let lit = item.kind.into_literal().unwrap();
                    match lit {
                        Integer(x) => sum += x as f64,
                        Float(x) => sum += x,
                        _ => panic!("bad type"),
                    }
                }

                ExprKind::Literal(Float(sum))
            }

            _ => {
                return Err(Error::new_simple(format!("unknown function {func_name}"))
                    .with_span(span)
                    .into())
            }
        })
    }

    fn eval_for_each_row(&mut self, relation: Expr, closure: Expr) -> Result<Expr> {
        // save relation from outer calls
        let prev_relation = self.relation.take();

        let relation_rows = relation.try_cast(|x| x.into_array(), None, "an array")?;
        let closure = closure
            .try_cast(|x| x.into_func(), None, "a function")?
            .body;

        // for every item in relation array, evaluate args
        let mut output_array = Vec::new();
        for relation_row in relation_rows {
            self.relation = Some(relation_row);

            output_array.push(self.fold_expr(*closure.clone())?);
        }

        // restore relation for outer calls
        self.relation = prev_relation;

        Ok(Expr::from(ExprKind::Array(output_array)))
    }

    fn eval_for_all_rows(&mut self, relation: Expr, closure: Expr) -> Result<Expr> {
        // save relation from outer calls
        let prev_relation = self.relation.take();

        let relation = columnar(relation)?;
        let closure = closure
            .try_cast(|x| x.into_func(), None, "a function")?
            .body;

        // eval other args
        self.relation = Some(relation);
        let res = self.fold_expr(*closure)?;

        // restore relation for outer calls
        self.relation = prev_relation;

        Ok(res)
    }
}

/// Converts `[{a = 1, b = false}, {a = 2, b = true}]`
/// into `{a = [1, 2], b = [false, true]}
fn columnar(expr: Expr) -> Result<Expr> {
    let relation_rows = expr.try_cast(|x| x.into_array(), None, "an array")?;

    let mut arg_tuple = Vec::new();
    for field in relation_rows.first().unwrap().kind.as_tuple().unwrap() {
        arg_tuple.push(Expr {
            alias: field.alias.clone(),
            ..Expr::from(ExprKind::Array(Vec::new()))
        });
    }

    // prepare output
    for relation_row in relation_rows {
        let fields = relation_row.try_cast(|x| x.into_tuple(), None, "a tuple")?;

        for (index, field) in fields.into_iter().enumerate() {
            arg_tuple[index].kind.as_array_mut().unwrap().push(field);
        }
    }
    Ok(Expr::from(ExprKind::Tuple(arg_tuple)))
}

fn std_module() -> Expr {
    Expr::from(ExprKind::Tuple(
        [
            new_func("floor", &["x"]),
            new_func("add", &["x", "y"]),
            new_func("neg", &["x"]),
            new_func("select", &["closure", "relation"]),
            new_func("derive", &["closure", "relation"]),
            new_func("filter", &["closure", "relation"]),
            new_func("aggregate", &["closure", "relation"]),
            new_func("sum", &["x"]),
        ]
        .to_vec(),
    ))
}

fn new_func(name: &str, params: &[&str]) -> Expr {
    let params = params
        .iter()
        .map(|name| FuncParam {
            name: name.to_string(),
            default_value: None,
            ty: None,
        })
        .collect();

    let kind = ExprKind::Func(Box::new(Func {
        name_hint: Some(Ident {
            path: vec!["std".to_string()],
            name: name.to_string(),
        }),

        // these don't matter
        return_ty: Default::default(),
        body: Box::new(Expr::null()),
        params,
        named_params: Default::default(),
        args: Default::default(),
        env: Default::default(),
    }));
    Expr {
        alias: Some(name.to_string()),
        ..Expr::from(kind)
    }
}

fn zip_relations(l: Expr, r: Expr) -> ExprKind {
    let l = l.kind.into_array().unwrap();
    let r = r.kind.into_array().unwrap();

    let mut res = Vec::new();
    for (l, r) in zip(l, r) {
        let l_fields = l.kind.into_tuple().unwrap();
        let r_fields = r.kind.into_tuple().unwrap();

        res.push(Expr::from(ExprKind::Tuple([l_fields, r_fields].concat())));
    }

    ExprKind::Array(res)
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

    #[test]
    fn transforms() {
        assert_display_snapshot!(eval(r"
            [
                { b = 4, c = false },
                { b = 5, c = true },
                { b = 12, c = true },
            ]
            std.select {c, b + 2}
            std.derive {d = 42}
            std.filter c
        ").unwrap(),
            @"[{c = true, 7, d = 42}, {c = true, 14, d = 42}]"
        );
    }
}
