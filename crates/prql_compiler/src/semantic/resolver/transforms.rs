use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::iter::zip;

use crate::ast::pl::expr::BinaryExpr;
use crate::ast::pl::fold::AstFold;
use crate::ast::pl::*;
use crate::error::{Error, Reason, WithErrorInfo};
use crate::generic::{SortDirection, WindowKind};

use super::super::context::{Decl, DeclKind};
use super::super::module::Module;
use super::Resolver;
use super::{Context, Lineage};
use super::{NS_PARAM, NS_THIS};

/// try to convert function call with enough args into transform
pub fn cast_transform(resolver: &mut Resolver, closure: Func) -> Result<Expr> {
    let internal_name = closure.body.kind.as_internal().unwrap();

    let (kind, input) = match internal_name.as_str() {
        "from" => {
            let [source] = unpack::<1>(closure);

            return Ok(source);
        }
        "select" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let tuple = resolver.coerce_into_tuple(assigns)?;
            (TransformKind::Select { tuple }, tbl)
        }
        "filter" => {
            let [filter, tbl] = unpack::<2>(closure);

            let filter = Box::new(filter);
            (TransformKind::Filter { filter }, tbl)
        }
        "derive" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let tuple = resolver.coerce_into_tuple(assigns)?;
            (TransformKind::Derive { tuple }, tbl)
        }
        "aggregate" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let tuple = resolver.coerce_into_tuple(assigns)?;
            (TransformKind::Aggregate { tuple }, tbl)
        }
        "sort" => {
            let [by, tbl] = unpack::<2>(closure);

            let by = resolver
                .coerce_into_tuple(by)?
                .kind
                .into_tuple()
                .unwrap()
                .into_iter()
                .map(|node| {
                    let (column, direction) = match node.kind {
                        ExprKind::RqOperator { name, mut args } if name == "std.neg" => {
                            (args.remove(0), SortDirection::Desc)
                        }
                        _ => (node, SortDirection::default()),
                    };
                    let column = Box::new(column);

                    ColumnSort { direction, column }
                })
                .collect();

            (TransformKind::Sort { by }, tbl)
        }
        "take" => {
            let [expr, tbl] = unpack::<2>(closure);

            let range = match expr.kind {
                ExprKind::Literal(Literal::Integer(n)) => range_from_ints(None, Some(n)),
                ExprKind::Range(range) => range,
                _ => {
                    return Err(Error::new(Reason::Expected {
                        who: Some("`take`".to_string()),
                        expected: "int or range".to_string(),
                        found: expr.to_string(),
                    })
                    // Possibly this should refer to the item after the `take` where
                    // one exists?
                    .with_span(expr.span)
                    .into());
                }
            };

            (TransformKind::Take { range }, tbl)
        }
        "join" => {
            let [side, with, filter, tbl] = unpack::<4>(closure);

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
            let [by, pipeline, tbl] = unpack::<3>(closure);

            let by = resolver.coerce_into_tuple(by)?;

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            let pipeline = Box::new(pipeline);
            (TransformKind::Group { by, pipeline }, tbl)
        }
        "window" => {
            let [rows, range, expanding, rolling, pipeline, tbl] = unpack::<6>(closure);

            let expanding = {
                let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: format!("{expanding}"),
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
                        found: format!("{rolling}"),
                    })
                    .with_span(rolling.span)
                })?
            };

            let rows = rows.try_cast(|r| r.into_range(), Some("parameter `rows`"), "a range")?;

            let range = range.try_cast(|r| r.into_range(), Some("parameter `range`"), "a range")?;

            let (kind, range) = if expanding {
                (WindowKind::Rows, range_from_ints(None, Some(0)))
            } else if rolling > 0 {
                (
                    WindowKind::Rows,
                    range_from_ints(Some(-rolling + 1), Some(0)),
                )
            } else if !range_is_empty(&rows) {
                (WindowKind::Rows, rows)
            } else if !range_is_empty(&range) {
                (WindowKind::Range, range)
            } else {
                (WindowKind::Rows, Range::unbounded())
            };

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            let transform_kind = TransformKind::Window {
                kind,
                range,
                pipeline: Box::new(pipeline),
            };
            (transform_kind, tbl)
        }
        "append" => {
            let [bottom, top] = unpack::<2>(closure);

            (TransformKind::Append(Box::new(bottom)), top)
        }
        "loop" => {
            let [pipeline, tbl] = unpack::<2>(closure);

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            (TransformKind::Loop(Box::new(pipeline)), tbl)
        }

        "in" => {
            // yes, this is not a transform, but this is the most appropriate place for it

            let [pattern, value] = unpack::<2>(closure);

            match pattern.kind {
                ExprKind::Range(Range { start, end }) => {
                    let start = start.map(|start| {
                        Expr::new(ExprKind::Binary(BinaryExpr {
                            left: Box::new(value.clone()),
                            op: BinOp::Gte,
                            right: start,
                        }))
                    });
                    let end = end.map(|end| {
                        Expr::new(ExprKind::Binary(BinaryExpr {
                            left: Box::new(value),
                            op: BinOp::Lte,
                            right: end,
                        }))
                    });

                    let res = new_binop(start, BinOp::And, end);
                    let res =
                        res.unwrap_or_else(|| Expr::new(ExprKind::Literal(Literal::Boolean(true))));
                    return Ok(res);
                }
                ExprKind::Tuple(_) => {
                    // TODO: should translate into `value IN (...)`
                    //   but RQ currently does not support sub queries or
                    //   even expressions that evaluate to a tuple.
                }
                _ => {}
            }
            return Err(Error::new(Reason::Expected {
                who: Some("std.in".to_string()),
                expected: "a pattern".to_string(),
                found: pattern.to_string(),
            })
            .with_span(pattern.span)
            .into());
        }

        "tuple_every" => {
            // yes, this is not a transform, but this is the most appropriate place for it

            let [list] = unpack::<1>(closure);
            let list = list.kind.into_tuple().unwrap();

            let mut res = None;
            for item in list {
                res = new_binop(res, BinOp::And, Some(item));
            }
            let res = res.unwrap_or_else(|| Expr::new(ExprKind::Literal(Literal::Boolean(true))));

            return Ok(res);
        }

        "tuple_map" => {
            // yes, this is not a transform, but this is the most appropriate place for it

            let [func, list] = unpack::<2>(closure);
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

            let [a, b] = unpack::<2>(closure);
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

            let [list] = unpack::<1>(closure);
            let list = list.kind.into_tuple().unwrap();
            let [a, b]: [Expr; 2] = list.try_into().unwrap();

            let res = new_binop(Some(a), BinOp::Eq, Some(b)).unwrap();
            return Ok(res);
        }

        "tuple_exclude" => {
            let [expr, exclude] = unpack(closure);

            return super::create_tuple_exclude(expr, exclude, None);
        }

        "from_text" => {
            // yes, this is not a transform, but this is the most appropriate place for it

            let [format, text_expr] = unpack::<2>(closure);

            let text = match text_expr.kind {
                ExprKind::Literal(Literal::String(text)) => text,
                _ => {
                    return Err(Error::new(Reason::Expected {
                        who: Some("std.from_text".to_string()),
                        expected: "a string literal".to_string(),
                        found: format!("`{text_expr}`"),
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

            let ty = resolver.declare_table_for_literal(expr_id, Some(columns), Some(input_name));

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
                ty: Some(ty),
                id: text_expr.id,
                ..res
            };
            return Ok(res);
        }

        _ => {
            return Err(Error::new_simple("unknown operator {internal_name}")
                .push_hint("this is a bug in prql-compiler")
                .with_span(closure.body.span)
                .into())
        }
    };

    let transform_call = TransformCall {
        kind: Box::new(kind),
        input: Box::new(input),
        partition: Vec::new(),
        frame: WindowFrame::default(),
        sort: Vec::new(),
    };
    Ok(Expr::new(ExprKind::TransformCall(transform_call)))
}

impl Resolver {
    /// Wraps non-tuple Exprs into a singleton Tuple.
    // This function should eventually be applied to all function arguments that
    // expect a tuple.
    pub fn coerce_into_tuple(&mut self, expr: Expr) -> Result<Expr> {
        Ok(if !expr.ty.as_ref().unwrap().is_tuple() {
            let expr = Expr::new(ExprKind::Tuple(vec![expr]));

            self.fold_expr(expr)?
        } else {
            if let Some(alias) = expr.alias {
                return Err(Error::new(Reason::Unexpected {
                    found: format!("assign to `{alias}`"),
                })
                .push_hint(format!("move assign into the tuple: `[{alias} = ...]`"))
                .with_span(expr.span)
                .into());
            }

            expr
        })
    }
}

fn range_is_empty(range: &Range) -> bool {
    fn as_int(bound: &Option<Box<Expr>>) -> Option<i64> {
        bound
            .as_ref()
            .and_then(|s| s.kind.as_literal())
            .and_then(|l| l.as_integer().cloned())
    }

    if let Some((s, e)) = as_int(&range.start).zip(as_int(&range.end)) {
        s >= e
    } else {
        false
    }
}

fn range_from_ints(start: Option<i64>, end: Option<i64>) -> Range {
    let start = start.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    let end = end.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    Range { start, end }
}

/// Simulate evaluation of the inner pipeline of group or window
// Creates a dummy node that acts as value that pipeline can be resolved upon.
fn fold_by_simulating_eval(
    resolver: &mut Resolver,
    pipeline: Expr,
    val_ty: Ty,
) -> Result<Expr, anyhow::Error> {
    log::debug!("fold by simulating evaluation");

    let param_name = "_tbl";
    let param_id = resolver.id.gen();
    let param_ty = Ty {
        lineage: Some(param_id),
        ..Ty::from(TyKind::Union(vec![]))
    };

    // resolver will not resolve a function call if any arguments are missing
    // but would instead return a closure to be resolved later.
    // because the pipeline of group is a function that takes a table chunk
    // and applies the transforms to it, it would not get resolved.
    // thats why we trick the resolver with a dummy node that acts as table
    // chunk and instruct resolver to apply the transform on that.

    let mut dummy = Expr::new(ExprKind::Ident(Ident::from_name(param_name)));
    dummy.ty = Some(val_ty);

    let pipeline = Expr::new(ExprKind::FuncCall(FuncCall::new_simple(
        pipeline,
        vec![dummy],
    )));

    let env = Module::singleton(param_name, Decl::from(DeclKind::Column(param_ty)));
    resolver.context.root_mod.stack_push(NS_PARAM, env);

    let pipeline = resolver.fold_expr(pipeline)?;

    resolver.context.root_mod.stack_pop(NS_PARAM).unwrap();

    // now, we need wrap the result into a closure and replace
    // the dummy node with closure's parameter.

    // extract reference to the dummy node
    // let mut tbl_node = extract_ref_to_first(&mut pipeline);
    // *tbl_node = Expr::new(ExprKind::Ident("x".to_string()));

    let pipeline = Expr::new(ExprKind::Func(Box::new(Func {
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
    })));
    Ok(pipeline)
}

impl Resolver {
    pub fn infer_type_of_transform(&mut self, transform: &TransformCall) -> Result<Ty> {
        use TransformKind::*;

        fn ty_relation_or_default(expr: &Expr) -> Ty {
            expr.ty
                .clone()
                .and_then(|t| t.into_relation())
                .or_else(|| {
                    Some(vec![TupleField::All {
                        ty: None,
                        exclude: HashSet::new(),
                    }])
                })
                .map(Ty::relation)
                .unwrap()
        }

        Ok(match transform.kind.as_ref() {
            Select { tuple } => {
                let tuple = tuple.ty.clone().unwrap();
                Ty::from(TyKind::Array(Box::new(tuple)))
            }
            Derive { tuple } => {
                let prev = ty_relation_or_default(&transform.input);
                let prev = prev.as_relation().unwrap();
                let new = tuple.ty.as_ref().unwrap().kind.as_tuple().unwrap();

                let combined = self.concat_tuples(prev, new)?;
                Ty::from(TyKind::Array(Box::new(combined)))
            }
            Aggregate { tuple } => {
                let tuple = tuple.ty.clone().unwrap();

                Ty::from(TyKind::Array(Box::new(tuple)))
            }
            Group { pipeline, by, .. } => {
                // pipeline's body is resolved, just use its type
                let Func { body, .. } = pipeline.kind.as_func().unwrap().as_ref();

                let mut ty = ty_relation_or_default(body);
                let fields = ty.as_relation_mut().unwrap();

                log::debug!("inferring type of group with pipeline: {body}");

                // prepend aggregate with `by` columns
                if let ExprKind::TransformCall(TransformCall { kind, .. }) = &body.as_ref().kind {
                    if let TransformKind::Aggregate { .. } = kind.as_ref() {
                        let by = by.ty.as_ref().unwrap().kind.as_tuple().unwrap();
                        log::debug!(".. group by {by:?}");

                        let combined = self.concat_tuples(by, fields)?;
                        ty = Ty::from(TyKind::Array(Box::new(combined)))
                    }
                }

                log::debug!(".. type={ty}");
                ty
            }
            Window { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Func { body, .. } = pipeline.kind.as_func().unwrap().as_ref();

                ty_relation_or_default(body)
            }
            Join { with, .. } => {
                let left = ty_relation_or_default(&transform.input);
                let right = ty_relation_or_default(with);

                join_relations(left, right)
            }
            Append(bottom) => {
                let top = ty_relation_or_default(&transform.input);
                let bottom = ty_relation_or_default(bottom);
                append_relations(top, bottom)?
            }
            Loop(_) => ty_relation_or_default(&transform.input),
            Sort { .. } | Filter { .. } | Take { .. } => ty_relation_or_default(&transform.input),
        })
    }

    fn concat_tuples(&mut self, a: &[TupleField], b: &[TupleField]) -> Result<Ty> {
        let new = Expr::new(ExprKind::Tuple(vec![
            Expr {
                ty: Some(Ty::from(TyKind::Tuple(a.to_vec()))),
                ..Expr::new(ExprKind::TupleFields(vec![]))
            },
            Expr {
                ty: Some(Ty::from(TyKind::Tuple(b.to_vec()))),
                ..Expr::new(ExprKind::TupleFields(vec![]))
            },
        ]));
        self.infer_type(&new).map(|x| x.unwrap())
    }
}

fn join_relations(mut lhs: Ty, rhs: Ty) -> Ty {
    let lhs_fields = lhs.as_relation_mut().unwrap();

    let rhs = rhs.into_relation().unwrap();
    lhs_fields.extend(rhs);

    lhs
}

fn append_relations(mut top: Ty, mut bottom: Ty) -> Result<Ty, Error> {
    let top_fields = top.as_relation_mut().unwrap();
    let bottom_fields = bottom.as_relation_mut().unwrap();

    if top_fields.len() != bottom_fields.len() {
        return Err(Error::new_simple(
            "cannot append two relations with non-matching number of columns.",
        ))
        .push_hint(format!(
            "top has {} columns, but bottom has {}",
            top_fields.len(),
            bottom_fields.len()
        ));
    }

    // TODO: I'm not sure what to use as input_name and expr_id...
    let mut fields = Vec::with_capacity(top_fields.len());
    for (t, b) in zip(top_fields.drain(..), bottom_fields.drain(..)) {
        fields.push(match (t, b) {
            (
                TupleField::All {
                    ty,
                    exclude: except,
                },
                TupleField::All { .. },
            ) => TupleField::All {
                ty,
                exclude: except,
            },
            (TupleField::Single(name_top, ty_top), TupleField::Single(name_bot, _)) => {
                let name = match (name_top, name_bot) {
                    (None, None) => None,
                    (None, Some(name)) | (Some(name), _) => Some(name),
                };

                TupleField::Single(name, ty_top)
            }
            (t, b) => {
                let msg = format!("cannot match columns `{t:?}` and `{b:?}`");
                let hint =
                    "make sure that top and bottom relations of append has the same column layout";
                return Err(Error::new_simple(msg).push_hint(hint));
            }
        });
    }

    top_fields.extend(fields);
    Ok(top)
}

impl Lineage {
    pub fn find_input(&self, input_name: &str) -> Option<&LineageInput> {
        self.inputs.iter().find(|i| i.name == input_name)
    }

    /// Renames all frame inputs to given alias.
    pub fn rename(&mut self, alias: String) {
        for input in &mut self.inputs {
            input.name = alias.clone();
        }

        for col in &mut self.columns {
            match col {
                LineageColumn::All { input_name, .. } => *input_name = alias.clone(),
                LineageColumn::Single {
                    name: Some(name), ..
                } => name.path = vec![alias.clone()],
                _ => {}
            }
        }
    }
}

// Expects closure's args to be resolved.
// Note that named args are before positional args, in order of declaration.
fn unpack<const P: usize>(closure: Func) -> [Expr; P] {
    closure.args.try_into().expect("bad transform cast")
}

mod from_text {
    use crate::ast::rq::RelationLiteral;

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
