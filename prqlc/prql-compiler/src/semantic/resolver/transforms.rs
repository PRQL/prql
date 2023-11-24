use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::iter::zip;

use prqlc_ast::error::{Error, Reason};
use prqlc_ast::{TupleField, Ty, TyKind};

use crate::ir::decl::{Decl, DeclKind, Module, RootModule};
use crate::ir::generic::{SortDirection, WindowKind};
use crate::ir::pl::PlFold;
use crate::ir::pl::*;
use crate::semantic::ast_expand::{restrict_null_literal, try_restrict_range};
use crate::semantic::resolver::functions::expr_of_func;
use crate::semantic::{write_pl, NS_PARAM, NS_THIS};
use crate::{WithErrorInfo, COMPILER_VERSION};

use super::types::{ty_tuple_kind, type_intersection};
use super::Resolver;

impl Resolver<'_> {
    /// try to convert function call with enough args into transform
    pub fn resolve_special_func(&mut self, func: Func) -> Result<Expr> {
        let internal_name = func.body.kind.into_internal().unwrap();

        let (kind, input) = match internal_name.as_str() {
            "from" => {
                let [source] = unpack::<1>(func.args);

                return Ok(source);
            }
            "select" => {
                let [assigns, tbl] = unpack::<2>(func.args);

                let assigns = Box::new(self.coerce_into_tuple(assigns)?);
                (TransformKind::Select { assigns }, tbl)
            }
            "filter" => {
                let [filter, tbl] = unpack::<2>(func.args);

                let filter = Box::new(filter);
                (TransformKind::Filter { filter }, tbl)
            }
            "derive" => {
                let [assigns, tbl] = unpack::<2>(func.args);

                let assigns = Box::new(self.coerce_into_tuple(assigns)?);
                (TransformKind::Derive { assigns }, tbl)
            }
            "aggregate" => {
                let [assigns, tbl] = unpack::<2>(func.args);

                let assigns = Box::new(self.coerce_into_tuple(assigns)?);
                (TransformKind::Aggregate { assigns }, tbl)
            }
            "sort" => {
                let [by, tbl] = unpack::<2>(func.args);

                let by = self
                    .coerce_into_tuple(by)?
                    .try_cast(|x| x.into_tuple(), Some("sort"), "tuple")?
                    .into_iter()
                    .map(|expr| {
                        let (column, direction) = match expr.kind {
                            ExprKind::RqOperator { name, mut args } if name == "std.neg" => {
                                (args.remove(0), SortDirection::Desc)
                            }
                            _ => (expr, SortDirection::default()),
                        };
                        let column = Box::new(column);

                        ColumnSort { direction, column }
                    })
                    .collect();

                (TransformKind::Sort { by }, tbl)
            }
            "take" => {
                let [expr, tbl] = unpack::<2>(func.args);

                let range = if let ExprKind::Literal(Literal::Integer(n)) = expr.kind {
                    range_from_ints(None, Some(n))
                } else {
                    match try_restrict_range(expr) {
                        Ok((start, end)) => Range {
                            start: restrict_null_literal(start).map(Box::new),
                            end: restrict_null_literal(end).map(Box::new),
                        },
                        Err(expr) => {
                            return Err(Error::new(Reason::Expected {
                                who: Some("`take`".to_string()),
                                expected: "int or range".to_string(),
                                found: write_pl(expr.clone()),
                            })
                            // Possibly this should refer to the item after the `take` where
                            // one exists?
                            .with_span(expr.span)
                            .into());
                        }
                    }
                };

                (TransformKind::Take { range }, tbl)
            }
            "join" => {
                let [side, with, filter, tbl] = unpack::<4>(func.args);

                let side = {
                    let span = side.span;
                    let ident = side.try_cast(ExprKind::into_ident, Some("side"), "ident")?;
                    match ident.to_string().as_str() {
                        "inner" => JoinSide::Inner,
                        "left" => JoinSide::Left,
                        "right" => JoinSide::Right,
                        "full" => JoinSide::Full,

                        found => bail!(Error::new(Reason::Expected {
                            who: Some("`side`".to_string()),
                            expected: "inner, left, right or full".to_string(),
                            found: found.to_string()
                        })
                        .with_span(span)),
                    }
                };

                let filter = Box::new(filter);
                let with = Box::new(with);
                (TransformKind::Join { side, with, filter }, tbl)
            }
            "group" => {
                let [by, pipeline, tbl] = unpack::<3>(func.args);

                let by = Box::new(self.coerce_into_tuple(by)?);

                // construct the relation that is passed into the pipeline
                // (when generics are a thing, this can be removed)
                let partition = {
                    let partition = Expr::new(ExprKind::All {
                        within: Box::new(Expr::new(Ident::from_name(NS_THIS))),
                        except: by.clone(),
                    });
                    // wrap into select, so the names are resolved correctly
                    let partition = FuncCall {
                        name: Box::new(Expr::new(Ident::from_path(vec!["std", "select"]))),
                        args: vec![partition, tbl],
                        named_args: Default::default(),
                    };
                    let partition = Expr::new(ExprKind::FuncCall(partition));
                    // fold, so lineage and types are inferred
                    self.fold_expr(partition)?
                };
                let pipeline = self.fold_by_simulating_eval(pipeline, &partition)?;

                // unpack tbl back out
                let tbl = *partition.kind.into_transform_call().unwrap().input;

                let pipeline = Box::new(pipeline);
                (TransformKind::Group { by, pipeline }, tbl)
            }
            "window" => {
                let [rows, range, expanding, rolling, pipeline, tbl] = unpack::<6>(func.args);

                let expanding = {
                    let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                    *as_bool.ok_or_else(|| {
                        Error::new(Reason::Expected {
                            who: Some("parameter `expanding`".to_string()),
                            expected: "a boolean".to_string(),
                            found: write_pl(expanding.clone()),
                        })
                        .with_span(expanding.span)
                    })?
                };

                let rolling = {
                    let as_int = rolling.kind.as_literal().and_then(|x| x.as_integer());

                    *as_int.ok_or_else(|| {
                        Error::new(Reason::Expected {
                            who: Some("parameter `rolling`".to_string()),
                            expected: "a number".to_string(),
                            found: write_pl(rolling.clone()),
                        })
                        .with_span(rolling.span)
                    })?
                };

                let rows = into_literal_range(try_restrict_range(rows).unwrap())?;

                let range = into_literal_range(try_restrict_range(range).unwrap())?;

                let (kind, start, end) = if expanding {
                    (WindowKind::Rows, None, Some(0))
                } else if rolling > 0 {
                    (WindowKind::Rows, Some(-rolling + 1), Some(0))
                } else if !range_is_empty(&rows) {
                    (WindowKind::Rows, rows.0, rows.1)
                } else if !range_is_empty(&range) {
                    (WindowKind::Range, range.0, range.1)
                } else {
                    (WindowKind::Rows, None, None)
                };
                // let start = Expr::new(start.map_or(Literal::Null, Literal::Integer));
                // let end = Expr::new(end.map_or(Literal::Null, Literal::Integer));
                let range = Range {
                    start: start.map(Literal::Integer).map(Expr::new).map(Box::new),
                    end: end.map(Literal::Integer).map(Expr::new).map(Box::new),
                };

                let pipeline = self.fold_by_simulating_eval(pipeline, &tbl)?;

                let transform_kind = TransformKind::Window {
                    kind,
                    range,
                    pipeline: Box::new(pipeline),
                };
                (transform_kind, tbl)
            }
            "append" => {
                let [bottom, top] = unpack::<2>(func.args);

                (TransformKind::Append(Box::new(bottom)), top)
            }
            "loop" => {
                let [pipeline, tbl] = unpack::<2>(func.args);

                let pipeline = self.fold_by_simulating_eval(pipeline, &tbl)?;

                (TransformKind::Loop(Box::new(pipeline)), tbl)
            }

            "in" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [pattern, value] = unpack::<2>(func.args);

                let pattern = match try_restrict_range(pattern) {
                    Ok((start, end)) => {
                        let start = restrict_null_literal(start);
                        let end = restrict_null_literal(end);

                        let start = start.map(|s| new_binop(value.clone(), &["std", "gte"], s));
                        let end = end.map(|e| new_binop(value, &["std", "lte"], e));

                        let res = maybe_binop(start, &["std", "and"], end);
                        let res = res.unwrap_or_else(|| {
                            Expr::new(ExprKind::Literal(Literal::Boolean(true)))
                        });
                        return Ok(res);
                    }
                    Err(expr) => expr,
                };

                return Err(Error::new(Reason::Expected {
                    who: Some("std.in".to_string()),
                    expected: "a pattern".to_string(),
                    found: write_pl(pattern.clone()),
                })
                .with_span(pattern.span)
                .into());
            }

            "tuple_every" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [list] = unpack::<1>(func.args);
                let list = list.kind.into_tuple().unwrap();

                let mut res = None;
                for item in list {
                    res = maybe_binop(res, &["std", "and"], Some(item));
                }
                let res =
                    res.unwrap_or_else(|| Expr::new(ExprKind::Literal(Literal::Boolean(true))));

                return Ok(res);
            }

            "tuple_map" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [func, list] = unpack::<2>(func.args);
                let list_items = list.kind.into_tuple().unwrap();

                let list_items = list_items
                    .into_iter()
                    .map(|item| {
                        Expr::new(ExprKind::FuncCall(FuncCall::new_simple(
                            func.clone(),
                            vec![item],
                        )))
                    })
                    .collect_vec();

                return Ok(Expr {
                    kind: ExprKind::Tuple(list_items),
                    ..list
                });
            }

            "tuple_zip" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [a, b] = unpack::<2>(func.args);
                let a = a.kind.into_tuple().unwrap();
                let b = b.kind.into_tuple().unwrap();

                let mut res = Vec::new();
                for (a, b) in std::iter::zip(a, b) {
                    res.push(Expr::new(ExprKind::Tuple(vec![a, b])));
                }

                return Ok(Expr::new(ExprKind::Tuple(res)));
            }

            "_eq" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [list] = unpack::<1>(func.args);
                let list = list.kind.into_tuple().unwrap();
                let [a, b]: [Expr; 2] = list.try_into().unwrap();

                let res = maybe_binop(Some(a), &["std", "eq"], Some(b)).unwrap();
                return Ok(res);
            }

            "from_text" => {
                // yes, this is not a transform, but this is the most appropriate place for it

                let [format, text_expr] = unpack::<2>(func.args);

                let text = match text_expr.kind {
                    ExprKind::Literal(Literal::String(text)) => text,
                    _ => {
                        return Err(Error::new(Reason::Expected {
                            who: Some("std.from_text".to_string()),
                            expected: "a string literal".to_string(),
                            found: format!("`{}`", write_pl(text_expr.clone())),
                        })
                        .with_span(text_expr.span)
                        .into());
                    }
                };

                let res = {
                    let span = format.span;
                    let format = format
                        .try_cast(ExprKind::into_ident, Some("format"), "ident")?
                        .to_string();
                    match format.as_str() {
                        "csv" => from_text::parse_csv(&text)?,
                        "json" => from_text::parse_json(&text)?,

                        _ => {
                            return Err(Error::new(Reason::Expected {
                                who: Some("`format`".to_string()),
                                expected: "csv or json".to_string(),
                                found: format,
                            })
                            .with_span(span)
                            .into())
                        }
                    }
                };

                let expr_id = text_expr.id.unwrap();
                let input_name = text_expr.alias.unwrap_or_else(|| "text".to_string());

                let columns: Vec<_> = res
                    .columns
                    .iter()
                    .cloned()
                    .map(|x| TupleField::Single(Some(x), None))
                    .collect();

                let frame =
                    self.declare_table_for_literal(expr_id, Some(columns), Some(input_name));

                let res = Expr::new(ExprKind::Array(
                    res.rows
                        .into_iter()
                        .map(|row| {
                            Expr::new(ExprKind::Tuple(
                                row.into_iter()
                                    .map(|lit| Expr::new(ExprKind::Literal(lit)))
                                    .collect(),
                            ))
                        })
                        .collect(),
                ));
                let res = Expr {
                    lineage: Some(frame),
                    id: text_expr.id,
                    ..res
                };
                return Ok(res);
            }

            "prql_version" => {
                // yes, this is not a transform, but this is the most appropriate place for it
                let ver = COMPILER_VERSION.to_string();
                return Ok(Expr::new(ExprKind::Literal(Literal::String(ver))));
            }

            _ => {
                return Err(
                    Error::new_simple(format!("unknown operator {internal_name}"))
                        .push_hint("this is a bug in prqlc")
                        .with_span(func.body.span)
                        .into(),
                )
            }
        };

        let transform_call = TransformCall {
            kind: Box::new(kind),
            input: Box::new(input),
            partition: None,
            frame: WindowFrame::default(),
            sort: Vec::new(),
        };
        let ty = self.infer_type_of_special_func(&transform_call)?;
        Ok(Expr {
            ty,
            ..Expr::new(ExprKind::TransformCall(transform_call))
        })
    }

    /// Wraps non-tuple Exprs into a singleton Tuple.
    pub(super) fn coerce_into_tuple(&mut self, expr: Expr) -> Result<Expr> {
        let is_tuple_ty = expr.ty.as_ref().unwrap().is_tuple() && !(expr.kind.is_all());
        Ok(if is_tuple_ty {
            // a helpful check for a common anti-pattern
            if let Some(alias) = expr.alias {
                return Err(Error::new(Reason::Unexpected {
                    found: format!("assign to `{alias}`"),
                })
                .push_hint(format!("move assign into the tuple: `[{alias} = ...]`"))
                .with_span(expr.span)
                .into());
            }

            expr
        } else {
            let span = expr.span;
            let mut expr = Expr::new(ExprKind::Tuple(vec![expr]));
            expr.span = span;

            self.fold_expr(expr)?
        })
    }

    /// Figure out the type of a function call, if this function is a *special function*.
    /// (declared in std module & requires special handling).
    pub fn infer_type_of_special_func(
        &mut self,
        transform_call: &TransformCall,
    ) -> Result<Option<Ty>> {
        // Long term plan is to make this function obsolete with generic function parameters.
        // In other words, I hope to make our type system powerful enough to express return
        // type of all std module functions.

        Ok(match transform_call.kind.as_ref() {
            TransformKind::Select { assigns } => assigns
                .ty
                .clone()
                .map(|x| Ty::new(TyKind::Array(Box::new(x)))),
            TransformKind::Derive { assigns } => {
                let input = transform_call.input.ty.clone().unwrap();
                let input = input.into_relation().unwrap();

                let derived = assigns.ty.clone().unwrap();
                let derived = derived.kind.into_tuple().unwrap();

                Some(Ty::new(TyKind::Array(Box::new(Ty::new(ty_tuple_kind(
                    [input, derived].concat(),
                ))))))
            }
            TransformKind::Aggregate { assigns } => {
                let tuple = assigns.ty.clone().unwrap();

                Some(Ty::new(TyKind::Array(Box::new(tuple))))
            }
            TransformKind::Filter { .. }
            | TransformKind::Sort { .. }
            | TransformKind::Take { .. } => transform_call.input.ty.clone(),
            TransformKind::Join { with, .. } => {
                let input = transform_call.input.ty.clone().unwrap();
                let input = input.into_relation().unwrap();

                let with_name = with.alias.clone();
                let with = with.ty.clone().unwrap();
                let with = with.kind.into_array().unwrap();
                let with = TupleField::Single(with_name, Some(*with));

                Some(Ty::new(TyKind::Array(Box::new(Ty::new(ty_tuple_kind(
                    [input, vec![with]].concat(),
                ))))))
            }
            TransformKind::Group { pipeline, by } => {
                let by = by.ty.clone().unwrap();
                let by = by.kind.into_tuple().unwrap();

                let pipeline = pipeline.ty.clone().unwrap();
                let pipeline = pipeline.kind.into_function().unwrap().unwrap();
                let pipeline = pipeline.return_ty.unwrap().into_relation().unwrap();

                Some(Ty::new(TyKind::Array(Box::new(Ty::new(ty_tuple_kind(
                    [by, pipeline].concat(),
                ))))))
            }
            TransformKind::Window { pipeline, .. } | TransformKind::Loop(pipeline) => {
                let pipeline = pipeline.ty.clone().unwrap();
                let pipeline = pipeline.kind.into_function().unwrap().unwrap();
                *pipeline.return_ty
            }
            TransformKind::Append(bottom) => {
                let top = transform_call.input.ty.clone().unwrap();
                let bottom = bottom.ty.clone().unwrap();

                Some(type_intersection(top, bottom))
            }
        })
    }
}

fn range_is_empty(range: &(Option<i64>, Option<i64>)) -> bool {
    match (&range.0, &range.1) {
        (Some(s), Some(e)) => s >= e,
        _ => false,
    }
}

fn range_from_ints(start: Option<i64>, end: Option<i64>) -> Range {
    let start = start.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    let end = end.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    Range { start, end }
}

fn into_literal_range(range: (Expr, Expr)) -> Result<(Option<i64>, Option<i64>)> {
    fn into_int(bound: Expr) -> Result<Option<i64>> {
        match bound.kind {
            ExprKind::Literal(Literal::Null) => Ok(None),
            ExprKind::Literal(Literal::Integer(i)) => Ok(Some(i)),
            _ => Err(Error::new_simple("expected an int literal")
                .with_span(bound.span)
                .into()),
        }
    }
    Ok((into_int(range.0)?, into_int(range.1)?))
}

impl Resolver<'_> {
    /// Simulate evaluation of the inner pipeline of group or window
    // Creates a dummy node that acts as value that pipeline can be resolved upon.
    fn fold_by_simulating_eval(
        &mut self,
        pipeline: Expr,
        val: &Expr,
    ) -> Result<Expr, anyhow::Error> {
        log::debug!("fold by simulating evaluation");
        let span = pipeline.span;

        let param_name = "_tbl";
        let param_id = self.id.gen();

        // resolver will not resolve a function call if any arguments are missing
        // but would instead return a closure to be resolved later.
        // because the pipeline of group is a function that takes a table chunk
        // and applies the transforms to it, it would not get resolved.
        // thats why we trick the resolver with a dummy node that acts as table
        // chunk and instruct resolver to apply the transform on that.

        let mut dummy = Expr::new(ExprKind::Ident(Ident::from_name(param_name)));
        dummy.lineage = val.lineage.clone();
        dummy.ty = val.ty.clone();

        let pipeline = Expr::new(ExprKind::FuncCall(FuncCall::new_simple(
            pipeline,
            vec![dummy],
        )));

        let env = Module::singleton(param_name, Decl::from(DeclKind::Column(param_id)));
        self.root_mod.module.stack_push(NS_PARAM, env);

        let mut pipeline = self.fold_expr(pipeline)?;

        self.root_mod.module.stack_pop(NS_PARAM).unwrap();

        // now, we need wrap the result into a closure and replace
        // the dummy node with closure's parameter.

        // extract reference to the dummy node
        // let mut tbl_node = extract_ref_to_first(&mut pipeline);
        // *tbl_node = Expr::new(ExprKind::Ident("x".to_string()));

        // validate that the return type is a relation
        // this can be removed after we have proper type checking for all std functions
        let expected = Some(Ty::relation(vec![TupleField::Wildcard(None)]));
        self.validate_expr_type(&mut pipeline, expected.as_ref(), &|| {
            Some("pipeline".to_string())
        })?;

        // construct the function back
        let func = Func {
            name_hint: None,
            body: Box::new(pipeline),
            return_ty: None,

            args: vec![],
            params: vec![FuncParam {
                name: param_id.to_string(),
                ty: None,
                default_value: None,
            }],
            named_params: vec![],

            env: Default::default(),
        };
        Ok(expr_of_func(func, span))
    }
}

impl TransformCall {
    pub fn infer_lineage(&self, root_mod: &RootModule) -> Result<Lineage> {
        use TransformKind::*;

        fn lineage_or_default(expr: &Expr) -> Result<Lineage> {
            expr.lineage
                .clone()
                .ok_or_else(|| anyhow!("expected {expr:?} to have table type"))
        }

        Ok(match self.kind.as_ref() {
            Select { assigns } => {
                let mut lineage = lineage_or_default(&self.input)?;

                lineage.clear();
                lineage.apply_assigns(assigns, root_mod);
                lineage
            }
            Derive { assigns } => {
                let mut lineage = lineage_or_default(&self.input)?;

                lineage.apply_assigns(assigns, root_mod);
                lineage
            }
            Group { pipeline, by, .. } => {
                let mut lineage = lineage_or_default(&self.input)?;
                lineage.clear();
                lineage.apply_assigns(by, root_mod);

                // pipeline's body is resolved, just use its type
                let Func { body, .. } = pipeline.kind.as_func().unwrap().as_ref();

                let partition_lin = lineage_or_default(body).unwrap();
                lineage.columns.extend(partition_lin.columns);

                log::debug!(".. type={lineage}");
                lineage
            }
            Window { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Func { body, .. } = pipeline.kind.as_func().unwrap().as_ref();

                lineage_or_default(body).unwrap()
            }
            Aggregate { assigns } => {
                let mut lineage = lineage_or_default(&self.input)?;
                lineage.clear();

                lineage.apply_assigns(assigns, root_mod);
                lineage
            }
            Join { with, .. } => {
                let left = lineage_or_default(&self.input)?;
                let right = lineage_or_default(with)?;
                join(left, right)
            }
            Append(bottom) => {
                let top = lineage_or_default(&self.input)?;
                let bottom = lineage_or_default(bottom)?;
                append(top, bottom)?
            }
            Loop(_) => lineage_or_default(&self.input)?,
            Sort { .. } | Filter { .. } | Take { .. } => lineage_or_default(&self.input)?,
        })
    }
}

fn join(mut lhs: Lineage, rhs: Lineage) -> Lineage {
    lhs.columns.extend(rhs.columns);
    lhs.inputs.extend(rhs.inputs);
    lhs
}

fn append(mut top: Lineage, bottom: Lineage) -> Result<Lineage, Error> {
    if top.columns.len() != bottom.columns.len() {
        return Err(Error::new_simple(
            "cannot append two relations with non-matching number of columns.",
        ))
        .push_hint(format!(
            "top has {} columns, but bottom has {}",
            top.columns.len(),
            bottom.columns.len()
        ));
    }

    // TODO: I'm not sure what to use as input_name and expr_id...
    let mut columns = Vec::with_capacity(top.columns.len());
    for (t, b) in zip(top.columns, bottom.columns) {
        columns.push(match (t, b) {
            (LineageColumn::All { input_id, except }, LineageColumn::All { .. }) => {
                LineageColumn::All { input_id, except }
            }
            (
                LineageColumn::Single {
                    name: name_t,
                    target_id,
                    target_name,
                },
                LineageColumn::Single { name: name_b, .. },
            ) => match (name_t, name_b) {
                (None, None) => {
                    let name = None;
                    LineageColumn::Single {
                        name,
                        target_id,
                        target_name,
                    }
                }
                (None, Some(name)) | (Some(name), _) => {
                    let name = Some(name);
                    LineageColumn::Single {
                        name,
                        target_id,
                        target_name,
                    }
                }
            },
            (t, b) => return Err(Error::new_simple(format!(
                "cannot match columns `{t:?}` and `{b:?}`"
            ))
            .push_hint(
                "make sure that top and bottom relations of append has the same column layout",
            )),
        });
    }

    top.columns = columns;
    Ok(top)
}

impl Lineage {
    pub fn clear(&mut self) {
        self.prev_columns.clear();
        self.prev_columns.append(&mut self.columns);
    }

    pub fn apply_assign(&mut self, expr: &Expr, root_mod: &RootModule) {
        // spacial case: all except
        if let ExprKind::All { except, .. } = &expr.kind {
            let except_exprs: HashSet<&usize> = except
                .kind
                .as_tuple()
                .iter()
                .flat_map(|x| x.iter())
                .flat_map(|e| e.target_id.iter())
                .collect();
            let except_inputs: HashSet<&usize> = except
                .kind
                .as_tuple()
                .iter()
                .flat_map(|x| x.iter())
                .flat_map(|e| e.target_ids.iter())
                .collect();

            for target_id in &expr.target_ids {
                let target_input = self.inputs.iter().find(|i| i.id == *target_id);
                match target_input {
                    Some(input) => {
                        // include all of the input's columns
                        if except_inputs.contains(target_id) {
                            continue;
                        }
                        self.columns.extend(input.get_all_columns(except, root_mod));
                    }
                    None => {
                        // include the column with if target_id
                        if except_exprs.contains(target_id) {
                            continue;
                        }
                        let prev_col = self.prev_columns.iter().find(|c| match c {
                            LineageColumn::Single {
                                target_id: expr_id, ..
                            } => expr_id == target_id,
                            _ => false,
                        });
                        self.columns.extend(prev_col.cloned());
                    }
                }
            }
            return;
        }

        // base case: append the column into the frame
        let id = expr.id.unwrap();

        let alias = expr.alias.as_ref();
        let name = alias
            .map(Ident::from_name)
            .or_else(|| expr.kind.as_ident().and_then(|i| i.clone().pop_front().1));

        // remove names from columns with the same name
        if name.is_some() {
            for c in &mut self.columns {
                if let LineageColumn::Single { name: n, .. } = c {
                    if n.as_ref().map(|i| &i.name) == name.as_ref().map(|i| &i.name) {
                        *n = None;
                    }
                }
            }
        }

        self.columns.push(LineageColumn::Single {
            name,
            target_id: id,
            target_name: None,
        });
    }

    pub fn apply_assigns(&mut self, assigns: &Expr, root_mod: &RootModule) {
        match &assigns.kind {
            ExprKind::Tuple(fields) => {
                for expr in fields {
                    self.apply_assigns(expr, root_mod);
                }
            }
            _ => self.apply_assign(assigns, root_mod),
        }
    }

    pub fn find_input_by_name(&self, input_name: &str) -> Option<&LineageInput> {
        self.inputs.iter().find(|i| i.name == input_name)
    }

    pub fn find_input(&self, input_id: usize) -> Option<&LineageInput> {
        self.inputs.iter().find(|i| i.id == input_id)
    }

    /// Renames all frame inputs to given alias.
    pub fn rename(&mut self, alias: String) {
        for input in &mut self.inputs {
            input.name = alias.clone();
        }

        for col in &mut self.columns {
            match col {
                LineageColumn::All { .. } => {}
                LineageColumn::Single {
                    name: Some(name), ..
                } => name.path = vec![alias.clone()],
                _ => {}
            }
        }
    }
}

impl LineageInput {
    fn get_all_columns(&self, except: &Expr, root_mod: &RootModule) -> Vec<LineageColumn> {
        let rel_def = root_mod.module.get(&self.table).unwrap();
        let rel_def = rel_def.kind.as_table_decl().unwrap();

        // TODO: can this panic?
        let columns = rel_def.ty.as_ref().unwrap().as_relation().unwrap();

        // special case: wildcard
        let has_wildcard = columns.iter().any(|c| matches!(c, TupleField::Wildcard(_)));
        if has_wildcard {
            // Relation has a wildcard (i.e. we don't know all the columns)
            // which means we cannot list all columns.
            // Instead we can just stick FrameColumn::All into the frame.
            // We could do this for all columns, but it is less transparent,
            // so let's use it just as a last resort.

            let input_ident_fq = Ident::from_path(vec![NS_THIS, self.name.as_str()]);

            let except = except
                .kind
                .as_tuple()
                .iter()
                .flat_map(|x| x.iter())
                .filter_map(|e| match &e.kind {
                    ExprKind::Ident(i) => Some(i),
                    _ => None,
                })
                .filter(|i| i.starts_with(&input_ident_fq))
                .map(|i| i.name.clone())
                .collect();

            return vec![LineageColumn::All {
                input_id: self.id,
                except,
            }];
        }

        // base case: convert rel_def into frame columns
        columns
            .iter()
            .map(|col| {
                let name = col.as_single().unwrap().0.clone().map(Ident::from_name);
                LineageColumn::Single {
                    name,
                    target_id: self.id,
                    target_name: None,
                }
            })
            .collect_vec()
    }
}

/// Expects closure's args to be resolved.
/// Note that named args are before positional args, in order of declaration.
fn unpack<const P: usize>(func_args: Vec<Expr>) -> [Expr; P] {
    func_args.try_into().expect("bad special function cast")
}

mod from_text {
    use crate::ir::rq::RelationLiteral;

    use super::*;

    // TODO: Can we dynamically get the types, like in pandas? We need to put
    // quotes around strings and not around numbers.
    // https://stackoverflow.com/questions/64369887/how-do-i-read-csv-data-without-knowing-the-structure-at-compile-time
    pub fn parse_csv(text: &str) -> Result<RelationLiteral> {
        let text = text.trim();
        let mut rdr = csv::Reader::from_reader(text.as_bytes());

        fn parse_header(row: &csv::StringRecord) -> Vec<String> {
            row.into_iter().map(|x| x.to_string()).collect()
        }

        fn parse_row(row: csv::StringRecord) -> Vec<Literal> {
            row.into_iter()
                .map(|x| Literal::String(x.to_string()))
                .collect()
        }

        Ok(RelationLiteral {
            columns: parse_header(rdr.headers()?),
            rows: rdr
                .records()
                .map(|row_result| row_result.map(parse_row))
                .try_collect()?,
        })
    }

    type JsonFormat1Row = HashMap<String, serde_json::Value>;

    #[derive(Deserialize)]
    struct JsonFormat2 {
        columns: Vec<String>,
        data: Vec<Vec<serde_json::Value>>,
    }

    fn map_json_primitive(primitive: serde_json::Value) -> Literal {
        use serde_json::Value::*;
        match primitive {
            Null => Literal::Null,
            Bool(bool) => Literal::Boolean(bool),
            Number(number) if number.is_i64() => Literal::Integer(number.as_i64().unwrap()),
            Number(number) if number.is_f64() => Literal::Float(number.as_f64().unwrap()),
            Number(_) => Literal::Null,
            String(string) => Literal::String(string),
            Array(_) => Literal::Null,
            Object(_) => Literal::Null,
        }
    }

    fn object_to_vec(
        mut row_map: HashMap<String, serde_json::Value>,
        columns: &[String],
    ) -> Vec<Literal> {
        columns
            .iter()
            .map(|c| {
                row_map
                    .remove(c)
                    .map(map_json_primitive)
                    .unwrap_or(Literal::Null)
            })
            .collect_vec()
    }

    pub fn parse_json(text: &str) -> Result<RelationLiteral> {
        parse_json1(text).or_else(|err1| {
            parse_json2(text)
                .map_err(|err2| anyhow!("While parsing rows: {err1}\nWhile parsing object: {err2}"))
        })
    }

    fn parse_json1(text: &str) -> Result<RelationLiteral> {
        let data: Vec<JsonFormat1Row> = serde_json::from_str(text)?;
        let mut columns = data
            .first()
            .ok_or_else(|| anyhow!("json: no rows"))?
            .keys()
            .cloned()
            .collect_vec();

        // JSON object keys are not ordered, so have to apply some order to produce
        // deterministic results
        columns.sort();

        let rows = data
            .into_iter()
            .map(|row_map| object_to_vec(row_map, &columns))
            .collect_vec();
        Ok(RelationLiteral { columns, rows })
    }

    fn parse_json2(text: &str) -> Result<RelationLiteral> {
        let JsonFormat2 { columns, data } = serde_json::from_str(text)?;

        Ok(RelationLiteral {
            columns,
            rows: data
                .into_iter()
                .map(|row| row.into_iter().map(map_json_primitive).collect_vec())
                .collect_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::semantic::test::parse_resolve_and_lower;

    #[test]
    fn test_aggregate_positional_arg() {
        // distinct query #292

        assert_yaml_snapshot!(parse_resolve_and_lower("
        from c_invoice
        select invoice_no
        group invoice_no (
            take 1
        )
        ").unwrap(), @r###"
        ---
        def:
          version: ~
          other: {}
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  - c_invoice
              columns:
                - Single: invoice_no
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Single: invoice_no
                      - 0
                    - - Wildcard
                      - 1
                  name: c_invoice
              - Select:
                  - 0
              - Take:
                  range:
                    start: ~
                    end:
                      kind:
                        Literal:
                          Integer: 1
                      span: ~
                  partition:
                    - 0
                  sort: []
              - Select:
                  - 0
          columns:
            - Single: invoice_no
        "###);

        // oops, two arguments #339
        let result = parse_resolve_and_lower(
            "
        from c_invoice
        aggregate average amount
        ",
        );
        assert!(result.is_err());

        // oops, two arguments
        let result = parse_resolve_and_lower(
            "
        from c_invoice
        group issued_at (aggregate average amount)
        ",
        );
        assert!(result.is_err());

        // correct function call
        let ctx = crate::semantic::test::parse_and_resolve(
            "
        from c_invoice
        group issued_at (
            aggregate (average amount)
        )
        ",
        )
        .unwrap();
        let (res, _) = ctx.find_main_rel(&[]).unwrap().clone();
        let expr = res.clone().into_relation_var().unwrap();
        let expr = super::super::test::erase_ids(*expr);
        assert_yaml_snapshot!(expr);
    }

    #[test]
    fn test_transform_sort() {
        assert_yaml_snapshot!(parse_resolve_and_lower("
        from invoices
        sort {issued_at, -amount, +num_of_articles}
        sort issued_at
        sort (-issued_at)
        sort {issued_at}
        sort {-issued_at}
        ").unwrap(), @r###"
        ---
        def:
          version: ~
          other: {}
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  - invoices
              columns:
                - Single: issued_at
                - Single: amount
                - Single: num_of_articles
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Single: issued_at
                      - 0
                    - - Single: amount
                      - 1
                    - - Single: num_of_articles
                      - 2
                    - - Wildcard
                      - 3
                  name: invoices
              - Sort:
                  - direction: Asc
                    column: 0
                  - direction: Desc
                    column: 1
                  - direction: Asc
                    column: 2
              - Sort:
                  - direction: Asc
                    column: 0
              - Sort:
                  - direction: Desc
                    column: 0
              - Sort:
                  - direction: Asc
                    column: 0
              - Sort:
                  - direction: Desc
                    column: 0
              - Select:
                  - 0
                  - 1
                  - 2
                  - 3
          columns:
            - Single: issued_at
            - Single: amount
            - Single: num_of_articles
            - Wildcard
        "###);
    }
}
