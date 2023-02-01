use std::cmp::Ordering;
use std::collections::hash_map::RandomState;
use std::collections::HashSet;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::ast::pl::{
    BinOp, ColumnSort, InterpolateItem, JoinSide, Literal, Range, WindowFrame, WindowKind,
};
use crate::ast::rq::{
    self, new_binop, CId, Compute, Expr, ExprKind, RqFold, TableRef, Transform, Window,
};
use crate::error::Error;
use crate::sql::context::AnchorContext;

use super::anchor::{infer_complexity, CidCollector, Complexity};
use super::Context;

#[derive(Debug, EnumAsInner, strum::AsRefStr)]
pub(super) enum SqlTransform {
    Super(Transform),
    Distinct,
    Except { bottom: TableRef, distinct: bool },
    Intersect { bottom: TableRef, distinct: bool },
    Union { bottom: TableRef, distinct: bool },
}

/// Pushes all [Transform::Select]s to the back of the pipeline.
pub(super) fn push_down_selects(pipeline: Vec<Transform>) -> Vec<Transform> {
    let mut select = None;
    let mut res = Vec::with_capacity(pipeline.len());
    for t in pipeline {
        if let Transform::Select(_) = t {
            select = Some(t);
        } else {
            res.push(t);
        }
    }
    if let Some(select) = select {
        res.push(select);
    }
    res
}

/// Removes unused relation inputs
pub(super) fn prune_inputs(mut pipeline: Vec<Transform>) -> Vec<Transform> {
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

pub(super) fn wrap(pipe: Vec<Transform>) -> Vec<SqlTransform> {
    pipe.into_iter().map(SqlTransform::Super).collect()
}

/// Creates [SqlTransform::Distinct] from [Transform::Take]
pub(super) fn distinct(
    pipeline: Vec<SqlTransform>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform>> {
    use SqlTransform::*;
    use Transform::*;

    let mut res = Vec::new();
    for transform in pipeline {
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
                if take_only_first && sort.is_empty() {
                    // TODO: use distinct only if `by == all columns in frame`
                    res.push(Distinct);
                    continue;
                }

                // convert `take range` into:
                //   derive _rn = s"ROW NUMBER"
                //   filter (_rn | in range)
                res.extend(create_filter_by_row_number(range, sort, partition, ctx));
            }
            _ => {
                res.push(transform);
            }
        }
    }
    Ok(res)
}

fn create_filter_by_row_number(
    range: Range<Expr>,
    sort: Vec<ColumnSort<CId>>,
    partition: Vec<CId>,
    ctx: &mut Context,
) -> Vec<SqlTransform> {
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
                    end: Some(*int_expr(0)),
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

    let col_ref = Box::new(Expr {
        kind: ExprKind::ColumnRef(compute.id),
        span: None,
    });

    // add the two transforms
    let range_int = range.try_map(as_int).unwrap();
    vec![
        SqlTransform::Super(Transform::Compute(compute)),
        SqlTransform::Super(Transform::Filter(match (range_int.start, range_int.end) {
            (Some(s), Some(e)) if s == e => Expr {
                span: None,
                kind: ExprKind::Binary {
                    left: col_ref,
                    op: BinOp::Eq,
                    right: int_expr(s),
                },
            },
            (start, end) => {
                let start = start.map(|start| Expr {
                    kind: ExprKind::Binary {
                        left: col_ref.clone(),
                        op: BinOp::Gte,
                        right: int_expr(start),
                    },
                    span: None,
                });
                let end = end.map(|end| Expr {
                    kind: ExprKind::Binary {
                        left: col_ref,
                        op: BinOp::Lte,
                        right: int_expr(end),
                    },
                    span: None,
                });

                let res = new_binop(start, BinOp::And, end);
                res.unwrap_or(Expr {
                    kind: ExprKind::Literal(Literal::Boolean(true)),
                    span: None,
                })
            }
        })),
    ]
}

fn as_int(expr: Expr) -> Result<i64, ()> {
    let lit = expr.kind.as_literal().ok_or(())?;
    lit.as_integer().cloned().ok_or(())
}

fn int_expr(i: i64) -> Box<Expr> {
    Box::new(Expr {
        span: None,
        kind: ExprKind::Literal(Literal::Integer(i)),
    })
}

/// Creates [SqlTransform::Union] from [Transform::Append]
pub(super) fn union(pipeline: Vec<SqlTransform>) -> Vec<SqlTransform> {
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
    res
}

/// Creates [SqlTransform::Except] from [Transform::Join] and [Transform::Filter]
pub(super) fn except(pipeline: Vec<SqlTransform>, ctx: &Context) -> Result<Vec<SqlTransform>> {
    use SqlTransform::*;
    use Transform::*;

    let output = AnchorContext::determine_select_columns(&pipeline);
    let output: HashSet<CId, RandomState> = HashSet::from_iter(output);

    let mut res = Vec::with_capacity(pipeline.len());
    for t in pipeline {
        res.push(t);

        if res.len() < 2 {
            continue;
        }
        let Super(Join { side: JoinSide::Left, filter: join_cond, with }) = &res[res.len() - 2] else { continue };
        let Super(Filter(filter)) = &res[res.len() - 1] else { continue };

        let top = AnchorContext::determine_select_columns(&res[0..res.len() - 2]);
        let bottom = with.columns.iter().map(|(_, c)| *c).collect_vec();

        // join_cond must be a join over all columns
        // (this could be loosened to check only the relation key)
        let (join_left, join_right) = collect_equals(join_cond);
        if !all_in(&top, join_left) || !all_in(&bottom, join_right) {
            continue;
        }

        // filter has to check for nullability of bottom
        // (this could be loosened to check only for nulls in a previously non-nullable column)
        let (filter_left, filter_right) = collect_equals(filter);
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
                return Err(Error::new_simple("Your dialect does not support EXCEPT ALL")
                    .with_help("If you provide more column information, your query can be translated to an anti join.")
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
pub(super) fn intersect(pipeline: Vec<SqlTransform>, ctx: &Context) -> Result<Vec<SqlTransform>> {
    use SqlTransform::*;
    use Transform::*;

    let output = AnchorContext::determine_select_columns(&pipeline);
    let output: HashSet<CId, RandomState> = HashSet::from_iter(output);

    let mut res = Vec::with_capacity(pipeline.len());
    let mut pipeline = pipeline.into_iter().peekable();
    while let Some(t) = pipeline.next() {
        res.push(t);

        if res.is_empty() {
            continue;
        }
        let Super(Join { side: JoinSide::Inner, filter: join_cond, with }) = &res[res.len() - 1] else { continue };

        let top = AnchorContext::determine_select_columns(&res[0..res.len() - 1]);
        let bottom = with.columns.iter().map(|(_, c)| *c).collect_vec();

        // join_cond must be a join over all columns
        // (this could be loosened to check only the relation key)
        let (left, right) = collect_equals(join_cond);
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
                return Err(Error::new_simple("Your dialect does not support INTERCEPT ALL")
                    .with_help("If you provide more column information, your query can be translated to an inner join.")
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
fn collect_equals(expr: &Expr) -> (Vec<&Expr>, Vec<&Expr>) {
    let mut lefts = Vec::new();
    let mut rights = Vec::new();

    match &expr.kind {
        ExprKind::Binary {
            left,
            op: BinOp::Eq,
            right,
        } => {
            lefts.push(left.as_ref());
            rights.push(right.as_ref());
        }
        ExprKind::Binary {
            left,
            op: BinOp::And,
            right,
        } => {
            let (l, r) = collect_equals(left);
            lefts.extend(l);
            rights.extend(r);

            let (l, r) = collect_equals(right);
            lefts.extend(l);
            rights.extend(r);
        }
        _ => {}
    }
    (lefts, rights)
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
pub(super) fn reorder(mut pipeline: Vec<SqlTransform>) -> Vec<SqlTransform> {
    use SqlTransform::*;
    use Transform::*;

    // reorder Compose
    pipeline.sort_by(|a, b| match (a, b) {
        // don't reorder with From or Join or itself
        (
            Super(From(_)) | Super(Join { .. }) | Super(Compute(_)),
            Super(From(_)) | Super(Join { .. }) | Super(Compute(_)),
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
pub(super) fn normalize(pipeline: Vec<Transform>) -> Vec<Transform> {
    Normalizer {}.fold_transforms(pipeline).unwrap()
}

struct Normalizer {}

impl RqFold for Normalizer {
    fn fold_expr_kind(&mut self, kind: ExprKind) -> Result<ExprKind> {
        let kind = rq::fold_expr_kind(self, kind)?;
        Ok(match kind {
            ExprKind::Binary {
                left,
                op: BinOp::Eq,
                right,
            } => {
                if let ExprKind::Literal(Literal::Null) = &left.kind {
                    ExprKind::Binary {
                        left: right,
                        op: BinOp::Eq,
                        right: left,
                    }
                } else {
                    ExprKind::Binary {
                        left,
                        op: BinOp::Eq,
                        right,
                    }
                }
            }
            kind => kind,
        })
    }
}

impl SqlTransform {
    pub fn as_str(&self) -> &str {
        match self {
            SqlTransform::Super(t) => t.as_ref(),
            _ => self.as_ref(),
        }
    }

    pub fn into_super_and<T, F: FnOnce(Transform) -> Result<T, Transform>>(
        self,
        f: F,
    ) -> Result<T, SqlTransform> {
        self.into_super()
            .and_then(|t| f(t).map_err(SqlTransform::Super))
    }
}

pub(super) trait SqlFold: RqFold {
    fn fold_sql_transforms(&mut self, transforms: Vec<SqlTransform>) -> Result<Vec<SqlTransform>> {
        transforms
            .into_iter()
            .map(|t| self.fold_sql_transform(t))
            .try_collect()
    }

    fn fold_sql_transform(&mut self, transform: SqlTransform) -> Result<SqlTransform> {
        Ok(match transform {
            SqlTransform::Super(t) => SqlTransform::Super(self.fold_transform(t)?),
            SqlTransform::Distinct => SqlTransform::Distinct,
            SqlTransform::Union { bottom, distinct } => SqlTransform::Union {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
            SqlTransform::Except { bottom, distinct } => SqlTransform::Except {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
            SqlTransform::Intersect { bottom, distinct } => SqlTransform::Intersect {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
        })
    }
}
