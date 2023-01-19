use std::cmp::Ordering;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::ast::pl::{BinOp, ColumnSort, InterpolateItem, Literal, Range, WindowFrame, WindowKind};
use crate::ast::rq::{self, new_binop, CId, Compute, Expr, ExprKind, RqFold, Transform, Window};

use super::anchor::{infer_complexity, Complexity};
use super::Context;

#[derive(Debug, EnumAsInner)]
pub(super) enum SqlTransform {
    Super(Transform),
    Distinct,
    // Except { distinct: bool },
    // Intersection { distinct: bool },
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

impl SqlTransform {
    pub fn as_str(&self) -> &str {
        match self {
            SqlTransform::Super(t) => t.as_ref(),
            SqlTransform::Distinct => "Distinct",
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
        })
    }
}
