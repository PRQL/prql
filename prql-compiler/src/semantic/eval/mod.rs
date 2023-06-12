use std::iter::zip;

use anyhow::Result;

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
        let has_for_each = (func.params.last())
            .filter(|x| x.name == "for_each")
            .is_some();
        let has_for_all = (func.params.last())
            .filter(|x| x.name == "for_all")
            .is_some();

        let args = if has_for_each {
            // save relation from outer calls
            let prev_relation = self.relation.take();

            let mut args = func.args;

            // eval relation
            let relation = args.pop().unwrap();
            let relation = self.fold_expr(relation)?;
            let relation = relation.try_cast(|x| x.into_array(), None, "an array")?;

            // prepare output
            let mut args_arrays = Vec::new();
            for _ in 0..(args.len() + 1) {
                args_arrays.push(Vec::new());
            }

            // for every item in relation array, evaluate args
            for relation_row in relation {
                self.relation = Some(relation_row);

                for (index, arg) in args.clone().into_iter().enumerate() {
                    args_arrays[index].push(self.fold_expr(arg)?);
                }

                args_arrays[args.len()].push(self.relation.take().unwrap());
            }

            // restore relation for outer calls
            self.relation = prev_relation;

            args_arrays
                .into_iter()
                .map(|array_items| Expr::from(ExprKind::Array(array_items)))
                .collect()
        } else if has_for_all {
            // save relation from outer calls
            let prev_relation = self.relation.take();

            let mut args = func.args;

            // eval relation
            let relation = args.pop().unwrap();
            let relation = self.fold_expr(relation)?;
            let relation_rows = relation
                .clone()
                .try_cast(|x| x.into_array(), None, "an array")?;

            // prepare output
            let mut args_tuple = Vec::new();
            for _ in 0..(args.len() + 1) {
                args_tuple.push(Vec::new());
            }
            for relation_row in relation_rows {
                let fields = relation_row.try_cast(|x| x.into_tuple(), None, "a tuple")?;

                for (index, field) in fields.into_iter().enumerate() {
                    args_tuple[index].push(field);
                }
            }
            let mut args_tuple_fields: Vec<_> = args_tuple
                .into_iter()
                .map(|array_items| Expr::from(ExprKind::Array(array_items)))
                .collect();
            for field in &mut args_tuple_fields {
                field.alias = (field.kind.as_array().unwrap())
                    .first()
                    .and_then(|x| x.alias.clone());
            }
            let args_tuple = Expr::from(ExprKind::Tuple(args_tuple_fields));

            // eval other args
            self.relation = Some(args_tuple);
            let mut args = self.fold_exprs(args)?;
            args.push(relation);

            // restore relation for outer calls
            self.relation = prev_relation;

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
                let [tuple, _relation]: [_; 2] = args.try_into().unwrap();
                tuple.kind
            }

            "std.derive" => {
                let [tuple, relation]: [_; 2] = args.try_into().unwrap();
                zip_relations(relation, tuple)
            }

            "std.filter" => {
                let [condition, relation]: [_; 2] = args.try_into().unwrap();

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
                let [tuple, _relation]: [_; 2] = args.try_into().unwrap();

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
}

fn std_module() -> Expr {
    Expr::from(ExprKind::Tuple(
        [
            new_func("floor", &["x"]),
            new_func("add", &["x", "y"]),
            new_func("neg", &["x"]),
            new_func("select", &["tuple", "for_each"]),
            new_func("derive", &["tuple", "for_each"]),
            new_func("filter", &["condition", "for_each"]),
            new_func("aggregate", &["tuple", "for_all"]),
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
