use std::iter::zip;

use anyhow::Result;
use itertools::Itertools;

use prqlc_ast::error::Error;
use prqlc_ast::Span;

use super::ast_expand;
use crate::ir::pl::{Expr, ExprKind, Func, FuncParam, Ident, Literal, PlFold};
use crate::WithErrorInfo;

pub fn eval(expr: prqlc_ast::expr::Expr) -> Result<Expr> {
    let expr = ast_expand::expand_expr(expr)?;

    Evaluator::new().fold_expr(expr)
}

/// Converts an expression to a value
///
/// Serves as a working draft of PRQL semantics definition.
struct Evaluator {
    context: Option<Expr>,
}

impl Evaluator {
    fn new() -> Self {
        Evaluator { context: None }
    }
}

impl PlFold for Evaluator {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        let mut expr = expr;

        expr.kind = match expr.kind {
            // these are values already
            ExprKind::Literal(l) => ExprKind::Literal(l),

            // these are values, iff their contents are values too
            ExprKind::Array(_) | ExprKind::Tuple(_) => self.fold_expr_kind(expr.kind)?,

            // functions are values
            ExprKind::Func(f) => ExprKind::Func(f),

            // ident are not values
            ExprKind::Ident(ident) => {
                // here we'd have to implement the whole name resolution, but for now,
                // let's do something simple

                // this is very crude, but for simple cases, it's enough
                let mut ident = ident;
                let mut base = self.context.clone();
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

            ExprKind::All { .. }
            | ExprKind::TransformCall(_)
            | ExprKind::SString(_)
            | ExprKind::FString(_)
            | ExprKind::Case(_)
            | ExprKind::RqOperator { .. }
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
                    // no evaluation
                    args.push(arg);
                } else {
                    // eval
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

                let relation = rows_to_cols(relation)?;
                let tuple = self.eval_within_context(tuple_closure, relation)?;

                ExprKind::Array(vec![tuple])
            }

            "std.window" => {
                let [tuple_closure, relation]: [_; 2] = args.try_into().unwrap();
                let relation_size = relation.kind.as_array().unwrap().len();
                let relation = rows_to_cols(relation)?;

                let mut res = Vec::new();

                const FRAME_ROWS: std::ops::Range<i64> = -1..1;

                for row_index in 0..relation_size {
                    let rel = windowed(relation.clone(), row_index, FRAME_ROWS, relation_size);

                    let row_value = self.eval_within_context(tuple_closure.clone(), rel)?;

                    res.push(row_value);
                }

                ExprKind::Array(res)
            }

            "std.columnar" => {
                let [relation_closure, relation]: [_; 2] = args.try_into().unwrap();
                let relation = rows_to_cols(relation)?;

                let res = self.eval_within_context(relation_closure, relation)?;

                cols_to_rows(res)?.kind
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

            "std.lag" => {
                let [array]: [_; 1] = args.try_into().unwrap();

                let mut array = array.try_cast(|x| x.into_array(), Some("lag"), "an array")?;

                if !array.is_empty() {
                    array.pop();
                    array.insert(0, Expr::new(Literal::Null));
                }

                ExprKind::Array(array)
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
        let prev_relation = self.context.take();

        let relation_rows = relation.try_cast(|x| x.into_array(), None, "an array")?;

        // for every item in relation array, evaluate args
        let mut output_array = Vec::new();
        for relation_row in relation_rows {
            let row_value = self.eval_within_context(closure.clone(), relation_row)?;
            output_array.push(row_value);
        }

        // restore relation for outer calls
        self.context = prev_relation;

        Ok(Expr::new(ExprKind::Array(output_array)))
    }

    fn eval_within_context(&mut self, expr: Expr, context: Expr) -> Result<Expr> {
        // save relation from outer calls
        let prev_relation = self.context.take();

        self.context = Some(context);
        let res = self.fold_expr(expr)?;

        // restore relation for outer calls
        self.context = prev_relation;

        Ok(res)
    }
}

fn windowed(
    mut relation: Expr,
    row_index: usize,
    frame: std::ops::Range<i64>,
    relation_size: usize,
) -> Expr {
    let row = row_index as i64;
    let end = (row + frame.end).clamp(0, relation_size as i64) as usize;
    let start = (row + frame.start).clamp(0, end as i64) as usize;

    for field in relation.kind.as_tuple_mut().unwrap() {
        let column = field.kind.as_array_mut().unwrap();

        column.drain(end..);
        column.drain(0..start);
    }
    relation
}

/// Converts `[{a = 1, b = false}, {a = 2, b = true}]`
/// into `{a = [1, 2], b = [false, true]}`
fn rows_to_cols(expr: Expr) -> Result<Expr> {
    let relation_rows = expr.try_cast(|x| x.into_array(), None, "an array")?;

    // prepare output
    let mut arg_tuple = Vec::new();
    for field in relation_rows.first().unwrap().kind.as_tuple().unwrap() {
        arg_tuple.push(Expr {
            alias: field.alias.clone(),
            ..Expr::new(ExprKind::Array(Vec::new()))
        });
    }

    // place entries
    for relation_row in relation_rows {
        let fields = relation_row.try_cast(|x| x.into_tuple(), None, "a tuple")?;

        for (index, field) in fields.into_iter().enumerate() {
            arg_tuple[index].kind.as_array_mut().unwrap().push(field);
        }
    }
    Ok(Expr::new(ExprKind::Tuple(arg_tuple)))
}

/// Converts `{a = [1, 2], b = [false, true]}`
/// into `[{a = 1, b = false}, {a = 2, b = true}]`
fn cols_to_rows(expr: Expr) -> Result<Expr> {
    let fields = expr.try_cast(|x| x.into_tuple(), None, "an tuple")?;

    let len = fields.first().unwrap().kind.as_array().unwrap().len();

    let mut rows = Vec::new();
    for index in 0..len {
        let mut row = Vec::new();
        for field in &fields {
            row.push(Expr {
                alias: field.alias.clone(),
                ..field.kind.as_array().unwrap()[index].clone()
            })
        }

        rows.push(Expr::new(ExprKind::Tuple(row)));
    }

    Ok(Expr::new(ExprKind::Array(rows)))
}

fn std_module() -> Expr {
    Expr::new(ExprKind::Tuple(
        [
            new_func("floor", &["x"]),
            new_func("add", &["x", "y"]),
            new_func("neg", &["x"]),
            new_func("select", &["closure", "relation"]),
            new_func("derive", &["closure", "relation"]),
            new_func("filter", &["closure", "relation"]),
            new_func("aggregate", &["closure", "relation"]),
            new_func("window", &["closure", "relation"]),
            new_func("columnar", &["closure", "relation"]),
            new_func("sum", &["x"]),
            new_func("lag", &["x"]),
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
        body: Box::new(Expr::new(Literal::Null)),
        params,
        named_params: Default::default(),
        args: Default::default(),
        env: Default::default(),
        generic_type_params: Default::default(),
    }));
    Expr {
        alias: Some(name.to_string()),
        ..Expr::new(kind)
    }
}

fn zip_relations(l: Expr, r: Expr) -> ExprKind {
    let l = l.kind.into_array().unwrap();
    let r = r.kind.into_array().unwrap();

    let mut res = Vec::new();
    for (l, r) in zip(l, r) {
        let l_fields = l.kind.into_tuple().unwrap();
        let r_fields = r.kind.into_tuple().unwrap();

        res.push(Expr::new(ExprKind::Tuple([l_fields, r_fields].concat())));
    }

    ExprKind::Array(res)
}

#[cfg(test)]
mod test {

    use insta::assert_display_snapshot;
    use itertools::Itertools;

    use crate::semantic::write_pl;

    use super::*;

    fn eval(source: &str) -> Result<String> {
        let stmts = crate::prql_to_pl(source)?.into_iter().exactly_one()?;
        let expr = stmts.kind.into_var_def().unwrap().value.unwrap();

        let value = super::eval(*expr)?;

        Ok(write_pl(value))
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

    #[test]
    fn window() {
        assert_display_snapshot!(eval(r"
            [
                { b = 4, c = false },
                { b = 5, c = true },
                { b = 12, c = true },
            ]
            std.window {d = std.sum b}
        ").unwrap(),
            @"[{d = 4}, {d = 9}, {d = 17}]"
        );
    }

    #[test]
    fn columnar() {
        assert_display_snapshot!(eval(r"
            [
                { b = 4, c = false },
                { b = 5, c = true },
                { b = 12, c = true },
            ]
            std.columnar {g = std.lag b}
        ").unwrap(),
            @"[{g = null}, {g = 4}, {g = 5}]"
        );
    }
}
