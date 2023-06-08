use std::cmp::Ordering;
use std::collections::hash_map::RandomState;
use std::collections::HashSet;

use anyhow::Result;
use itertools::Itertools;

use crate::ast::pl::{
    ColumnSort, InterpolateItem, JoinSide, Literal, Range, WindowFrame, WindowKind,
};
use crate::ast::rq::{
    self, maybe_binop, new_binop, CId, Compute, Expr, ExprKind, RqFold, TableRef, Transform, Window,
};
use crate::error::{Error, WithErrorInfo};
use crate::sql::Context;

use super::anchor::{infer_complexity, CidCollector, Complexity};
use super::ast::*;
use super::context::AnchorContext;

/// Converts RQ AST into SqlRQ AST and applies a few preprocessing operations.
///
/// Note that some SQL translation mechanisms depend on behavior of some of these
/// functions (i.e. reorder).
pub(in crate::sql) fn preprocess(
    pipeline: Vec<Transform>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform<TableRef>>, anyhow::Error> {
    Ok(pipeline)
        .and_then(normalize)
        .map(prune_inputs)
        .map(wrap)
        .and_then(|p| distinct(p, ctx))
        .and_then(union)
        .and_then(|p| except(p, ctx))
        .and_then(|p| intersect(p, ctx))
        .map(reorder)
}

// This function was disabled because it changes semantics of the pipeline in some cases.
// /// Pushes all [Transform::Select]s to the back of the pipeline.
// pub(in crate::sql) fn push_down_selects(pipeline: Vec<Transform>) -> Vec<Transform> {
//     let mut select = None;
//     let mut res = Vec::with_capacity(pipeline.len());
//     for t in pipeline {
//         if let Transform::Select(_) = t {
//             select = Some(t);
//         } else {
//             res.push(t);
//         }
//     }
//     if let Some(select) = select {
//         res.push(select);
//     }
//     res
// }

/// Removes unused relation inputs
pub(in crate::sql) fn prune_inputs(mut pipeline: Vec<Transform>) -> Vec<Transform> {
    let mut used_cids = HashSet::new();

    let mut res = Vec::new();
    while let Some(mut transform) = pipeline.pop() {
        // collect cids (special case for Join & From)
        match &transform {
            Transform::Join { filter, .. } => {
                used_cids.extend(CidCollector::collect(filter.clone()));
            }
            Transform::From(_) => {}
            _ => {
                let (t, cids) = CidCollector::collect_t(transform);
                used_cids.extend(cids);
                transform = t;
            }
        }

        // prune unused inputs
        if let Transform::From(with) | Transform::Join { with, .. } = &mut transform {
            with.columns.retain(|(_, cid)| used_cids.contains(cid));
        }

        res.push(transform);
    }

    res.reverse();
    res
}

pub(in crate::sql) fn wrap(pipe: Vec<Transform>) -> Vec<SqlTransform<TableRef>> {
    pipe.into_iter().map(SqlTransform::Super).collect()
}

fn vecs_contain_same_elements<T: Eq + std::hash::Hash>(a: &[T], b: &[T]) -> bool {
    let a: HashSet<&T, RandomState> = a.iter().collect();
    let b: HashSet<&T, RandomState> = b.iter().collect();
    a == b
}

/// Creates [SqlTransform::Distinct] from [Transform::Take]
pub(in crate::sql) fn distinct(
    pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform<TableRef>>> {
    use SqlTransform::Super;
    use Transform::*;

    let mut res = Vec::new();
    for transform in pipeline.clone() {
        match transform {
            Super(Take(rq::Take { ref partition, .. })) if partition.is_empty() => {
                res.push(transform);
            }

            Super(Take(rq::Take {
                range,
                partition,
                sort,
            })) => {
                let range_int = range
                    .clone()
                    .try_map(as_int)
                    .map_err(|_| anyhow::anyhow!("Invalid take arguments"))?;

                let take_only_first =
                    range_int.start.unwrap_or(1) == 1 && matches!(range_int.end, Some(1));

                // Check whether the columns within the partition are the same
                // as the columns in the table; otherwise we can't use DISTINCT.
                let columns_in_frame = AnchorContext::determine_select_columns(&pipeline.clone());
                let matching_columns = vecs_contain_same_elements(&columns_in_frame, &partition);

                if take_only_first && sort.is_empty() && matching_columns {
                    // DISTINCT

                    res.push(SqlTransform::Distinct);
                } else if ctx.dialect.supports_distinct_on() {
                    // DISTINCT ON

                    let sort = if sort.is_empty() {
                        vec![]
                    } else {
                        [into_column_sort(&partition), sort].concat()
                    };

                    res.push(SqlTransform::Sort(sort));
                    res.push(SqlTransform::DistinctOn(partition));
                } else {
                    // convert `take range` into:
                    //   derive _rn = s"ROW NUMBER"
                    //   filter (_rn | in range)
                    res.extend(create_filter_by_row_number(range, sort, partition, ctx));
                }
            }
            _ => {
                res.push(transform);
            }
        }
    }
    Ok(res)
}

fn into_column_sort(partition: &[CId]) -> Vec<ColumnSort<CId>> {
    partition
        .iter()
        .map(|cid| ColumnSort {
            direction: crate::ast::pl::SortDirection::Asc,
            column: *cid,
        })
        .collect_vec()
}

fn create_filter_by_row_number(
    range: Range<Expr>,
    sort: Vec<ColumnSort<CId>>,
    partition: Vec<CId>,
    ctx: &mut Context,
) -> Vec<SqlTransform<TableRef>> {
    // declare new column
    let expr = Expr {
        kind: ExprKind::SString(vec![InterpolateItem::String("ROW_NUMBER()".to_string())]),
        span: None,
    };

    let is_unsorted = sort.is_empty();
    let window = Window {
        frame: if is_unsorted {
            WindowFrame {
                kind: WindowKind::Rows,
                range: Range::unbounded(),
            }
        } else {
            WindowFrame {
                kind: WindowKind::Range,
                range: Range {
                    start: None,
                    end: Some(int_expr(0)),
                },
            }
        },
        partition,
        sort,
    };

    let compute = Compute {
        id: ctx.anchor.cid.gen(),
        expr,
        window: Some(window),
        is_aggregation: false,
    };

    ctx.anchor.register_compute(compute.clone());

    let col_ref = Expr {
        kind: ExprKind::ColumnRef(compute.id),
        span: None,
    };

    // add the two transforms
    let range_int = range.try_map(as_int).unwrap();

    let compute = SqlTransform::Super(Transform::Compute(compute));
    let filter = SqlTransform::Super(Transform::Filter(match (range_int.start, range_int.end) {
        (Some(s), Some(e)) if s == e => new_binop(col_ref, "std.eq", int_expr(s)),
        (start, end) => {
            let start = start.map(|start| new_binop(col_ref.clone(), "std.gte", int_expr(start)));
            let end = end.map(|end| new_binop(col_ref, "std.lte", int_expr(end)));

            maybe_binop(start, "std.and", end).unwrap_or(Expr {
                kind: ExprKind::Literal(Literal::Boolean(true)),
                span: None,
            })
        }
    }));

    vec![compute, filter]
}

fn as_int(expr: Expr) -> Result<i64, ()> {
    let lit = expr.kind.as_literal().ok_or(())?;
    lit.as_integer().cloned().ok_or(())
}

fn int_expr(i: i64) -> Expr {
    Expr {
        span: None,
        kind: ExprKind::Literal(Literal::Integer(i)),
    }
}

/// Creates [SqlTransform::Union] from [Transform::Append]
pub(in crate::sql) fn union(
    pipeline: Vec<SqlTransform<TableRef>>,
) -> Result<Vec<SqlTransform<TableRef>>> {
    use SqlTransform::*;
    use Transform::*;

    let mut res = Vec::with_capacity(pipeline.len());
    let mut pipeline = pipeline.into_iter().peekable();
    while let Some(t) = pipeline.next() {
        let Super(Append(bottom)) = t else {
            res.push(t);
            continue;
        };

        let distinct = if let Some(Distinct) = &pipeline.peek() {
            pipeline.next();
            true
        } else {
            false
        };

        res.push(SqlTransform::Union { bottom, distinct });
    }
    Ok(res)
}

/// Creates [SqlTransform::Except] from [Transform::Join] and [Transform::Filter]
pub(in crate::sql) fn except(
    pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform<TableRef>>> {
    use SqlTransform::*;

    let output = AnchorContext::determine_select_columns(&pipeline);
    let output: HashSet<CId, RandomState> = HashSet::from_iter(output);

    let mut res = Vec::with_capacity(pipeline.len());
    for t in pipeline {
        res.push(t);

        if res.len() < 2 {
            continue;
        }
        let Super(Transform::Join { side: JoinSide::Left, filter: join_cond, with }) = &res[res.len() - 2] else { continue };
        let Super(Transform::Filter(filter)) = &res[res.len() - 1] else { continue };

        let top = AnchorContext::determine_select_columns(&res[0..res.len() - 2]);
        let bottom = with.columns.iter().map(|(_, c)| *c).collect_vec();

        // join_cond must be a join over all columns
        // (this could be loosened to check only the relation key)
        let (join_left, join_right) = collect_equals(join_cond)?;
        if !all_in(&top, join_left) || !all_in(&bottom, join_right) {
            continue;
        }

        // filter has to check for nullability of bottom
        // (this could be loosened to check only for nulls in a previously non-nullable column)
        let (filter_left, filter_right) = collect_equals(filter)?;
        if !(all_in(&bottom, filter_left) && all_null(filter_right)) {
            continue;
        }

        // select must not contain things from bottom
        if bottom.iter().any(|c| output.contains(c)) {
            continue;
        }

        // determine DISTINCT
        let mut distinct = false;
        // EXCEPT ALL can become except EXCEPT DISTINCT, if top is DISTINCT.
        // DISTINCT-ness of bottom has no effect on the output.
        if res.len() >= 3 {
            if let Distinct = &res[res.len() - 3] {
                distinct = true;
            }
        }

        if !distinct && !ctx.dialect.except_all() {
            // EXCEPT ALL is not supported
            // can we fall back to anti-join?
            if ctx.anchor.contains_wildcard(&top) || ctx.anchor.contains_wildcard(&bottom) {
                return Err(Error::new_simple(format!("The dialect {:?} does not support EXCEPT ALL", ctx.dialect))
                    .push_hint("Providing more column information will allow the query to be translated to an anti-join.")
                    .into());
            } else {
                // Don't create Except, fallback to anti-join.
                continue;
            }
        }

        res.pop(); // filter
        let join = res.pop(); // join
        let (_, with, _) = join.unwrap().into_super().unwrap().into_join().unwrap();
        if distinct {
            if let Some(Distinct) = &res.last() {
                res.pop();
            }
        }

        res.push(SqlTransform::Except {
            bottom: with,
            distinct,
        });
    }

    Ok(res)
}

/// Creates [SqlTransform::Intersect] from [Transform::Join]
pub(in crate::sql) fn intersect(
    pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform<TableRef>>> {
    use SqlTransform::*;

    let output = AnchorContext::determine_select_columns(&pipeline);
    let output: HashSet<CId, RandomState> = HashSet::from_iter(output);

    let mut res = Vec::with_capacity(pipeline.len());
    let mut pipeline = pipeline.into_iter().peekable();
    while let Some(t) = pipeline.next() {
        res.push(t);

        if res.is_empty() {
            continue;
        }
        let Super(Transform::Join { side: JoinSide::Inner, filter: join_cond, with }) = &res[res.len() - 1] else { continue };

        let top = AnchorContext::determine_select_columns(&res[0..res.len() - 1]);
        let bottom = with.columns.iter().map(|(_, c)| *c).collect_vec();

        // join_cond must be a join over all columns
        // (this could be loosened to check only the relation key)
        let (left, right) = collect_equals(join_cond)?;
        if !(all_in(&top, left) && all_in(&bottom, right)) {
            continue;
        }

        // select must not contain things from bottom
        if bottom.iter().any(|c| output.contains(c)) {
            continue;
        }

        // determine DISTINCT
        let mut distinct = false;
        // INTERSECT ALL can become except INTERSECT DISTINCT
        // - if top is DISTINCT or
        // - if output is DISTINCT
        if res.len() > 1 {
            if let Distinct = &res[res.len() - 2] {
                distinct = true;
            }
        }
        if let Some(SqlTransform::Distinct) = pipeline.peek() {
            distinct = true;
        }

        if !distinct && !ctx.dialect.intersect_all() {
            // INTERCEPT ALL is not supported
            // can we fall back to anti-join?
            if ctx.anchor.contains_wildcard(&top) || ctx.anchor.contains_wildcard(&bottom) {
                return Err(Error::new_simple(format!("The dialect {:?} does not support INTERSECT ALL", ctx.dialect))
                    .push_hint("Providing more column information will allow the query to be translated to an anti-join.")
                    .into());
            } else {
                // Don't create Intercept, fallback to inner join.
                continue;
            }
        }

        // remove "used up transforms"
        let join = res.pop(); // join
        let (_, with, _) = join.unwrap().into_super().unwrap().into_join().unwrap();
        if distinct {
            if let Some(Distinct) = &res.last() {
                res.pop();
            }
            if let Some(SqlTransform::Distinct) = pipeline.peek() {
                pipeline.next();
            }
        }

        // push the new transform
        res.push(SqlTransform::Intersect {
            bottom: with,
            distinct,
        });
    }

    Ok(res)
}

/// Returns true if all cids are in exprs
fn all_in(cids: &[CId], exprs: Vec<&Expr>) -> bool {
    let exprs = col_refs(exprs);
    cids.iter().all(|c| exprs.contains(c))
}

fn all_null(exprs: Vec<&Expr>) -> bool {
    exprs
        .iter()
        .all(|e| matches!(e.kind, ExprKind::Literal(Literal::Null)))
}

/// Converts `(a == b) and ((c == d) and (e == f))`
/// into `([a, c, e], [b, d, f])`
fn collect_equals(expr: &Expr) -> Result<(Vec<&Expr>, Vec<&Expr>)> {
    let mut lefts = Vec::new();
    let mut rights = Vec::new();

    match &expr.kind {
        ExprKind::Operator { name, args } if name == "std.eq" && args.len() == 2 => {
            lefts.push(&args[0]);
            rights.push(&args[1]);
        }
        ExprKind::Operator { name, args } if name == "std.and" && args.len() == 2 => {
            let (l, r) = collect_equals(&args[0])?;
            lefts.extend(l);
            rights.extend(r);

            let (l, r) = collect_equals(&args[1])?;
            lefts.extend(l);
            rights.extend(r);
        }
        _ => (),
    }

    Ok((lefts, rights))
}

fn col_refs(exprs: Vec<&Expr>) -> Vec<CId> {
    exprs
        .into_iter()
        .flat_map(|expr| expr.kind.as_column_ref().cloned())
        .collect()
}

/// Pull Compose transforms in front of other transforms if possible.
/// Position of Compose is important for two reasons:
/// - when splitting pipelines, they provide information in which pipeline the
///   column is computed and subsequently, with which table name should be used
///   for name materialization.
/// - the transform order in SQL requires Computes to be before Filter. This
///   can be circumvented by materializing the column earlier in the pipeline,
///   which is done in this function.
pub(in crate::sql) fn reorder(
    mut pipeline: Vec<SqlTransform<TableRef>>,
) -> Vec<SqlTransform<TableRef>> {
    use SqlTransform::Super;
    use Transform::*;

    // reorder Compose
    pipeline.sort_by(|a, b| match (a, b) {
        // don't reorder with From or Join or itself
        (
            Super(Transform::From(_)) | Super(Transform::Join { .. }) | Super(Compute(_)),
            Super(Transform::From(_)) | Super(Transform::Join { .. }) | Super(Compute(_)),
        ) => Ordering::Equal,

        // reorder always
        (Super(Sort(_)), Super(Compute(_))) => Ordering::Greater,
        (Super(Compute(_)), Super(Sort(_))) => Ordering::Less,

        // reorder if col decl is plain
        (Super(Take(_)), Super(Compute(decl))) if infer_complexity(decl) == Complexity::Plain => {
            Ordering::Greater
        }
        (Super(Compute(decl)), Super(Take(_))) if infer_complexity(decl) == Complexity::Plain => {
            Ordering::Less
        }

        // don't reorder by default
        _ => Ordering::Equal,
    });

    pipeline
}

/// Normalize query:
/// - Swap null checks such that null is always on the right side.
///   This is needed to simplify code for Except and for compiling to IS NULL.
pub(in crate::sql) fn normalize(pipeline: Vec<Transform>) -> Result<Vec<Transform>> {
    Normalizer {}.fold_transforms(pipeline)
}

struct Normalizer {}

impl RqFold for Normalizer {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        let expr = Expr {
            kind: rq::fold_expr_kind(self, expr.kind)?,
            ..expr
        };

        if let ExprKind::Operator { name, args } = &expr.kind {
            if name == "std.eq" && args.len() == 2 {
                let (left, right) = (&args[0], &args[1]);
                let span = expr.span;
                let new_args = if let ExprKind::Literal(Literal::Null) = &left.kind {
                    vec![right.clone(), left.clone()]
                } else {
                    vec![left.clone(), right.clone()]
                };
                let new_kind = ExprKind::Operator {
                    name: name.clone(),
                    args: new_args,
                };
                return Ok(Expr {
                    kind: new_kind,
                    span,
                });
            }
        }

        Ok(expr)
    }
}
