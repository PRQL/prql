use std::cmp::Ordering;

use anyhow::Result;

use crate::ast::pl::{BinOp, ColumnSort, InterpolateItem, Literal, Range, WindowFrame, WindowKind};
use crate::ast::rq::{new_binop, CId, Compute, Expr, ExprKind, RqFold, Take, Transform, Window};

use super::anchor::{infer_complexity, Complexity};
use super::context::AnchorContext;
use super::Context;

pub(super) fn preprocess_distinct(
    pipeline: Vec<Transform>,
    context: &mut Context,
) -> Result<Vec<Transform>> {
    let mut d = TakeConverter {
        context: &mut context.anchor,
    };
    d.fold_transforms(pipeline)
}
/// Creates [Transform::Unique] from [Transform::Take]
struct TakeConverter<'a> {
    context: &'a mut AnchorContext,
}

impl<'a> RqFold for TakeConverter<'a> {
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> Result<Vec<Transform>> {
        let mut res = Vec::new();

        for transform in transforms {
            match transform {
                Transform::Take(Take { ref partition, .. }) if partition.is_empty() => {
                    res.push(transform);
                }

                Transform::Take(Take {
                    range,
                    partition,
                    sort,
                }) => {
                    let range_int = range
                        .clone()
                        .try_map(as_int)
                        .map_err(|_| anyhow::anyhow!("Invalid take arguments"))?;

                    let take_only_first =
                        range_int.start.unwrap_or(1) == 1 && matches!(range_int.end, Some(1));
                    if take_only_first && sort.is_empty() {
                        // TODO: use distinct only if `by == all columns in frame`
                        res.push(Transform::Unique);
                        continue;
                    }

                    // convert `take range` into:
                    //   derive _rn = s"ROW NUMBER"
                    //   filter (_rn | in range)
                    res.extend(self.create_filter_by_row_number(range, sort, partition));
                }
                _ => {
                    res.push(transform);
                }
            }
        }
        Ok(res)
    }
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

impl<'a> TakeConverter<'a> {
    fn create_filter_by_row_number(
        &mut self,
        range: Range<Expr>,
        sort: Vec<ColumnSort<CId>>,
        partition: Vec<CId>,
    ) -> Vec<Transform> {
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
            id: self.context.cid.gen(),
            expr,
            window: Some(window),
            is_aggregation: false,
        };

        self.context.register_compute(compute.clone());

        let col_ref = Box::new(Expr {
            kind: ExprKind::ColumnRef(compute.id),
            span: None,
        });

        // add the two transforms
        let range_int = range.try_map(as_int).unwrap();
        vec![
            Transform::Compute(compute),
            Transform::Filter(match (range_int.start, range_int.end) {
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
            }),
        ]
    }
}

/// Pull Compose transforms in front of other transforms if possible.
/// Position of Compose is important for two reasons:
/// - when splitting pipelines, they provide information in which pipeline the
///   column is computed and subsequently, with which table name should be used
///   for name materialization.
/// - the transform order in SQL requires Computes to be before Filter. This
///   can be circumvented by materializing the column earlier in the pipeline,
///   which is done in this function.
pub(super) fn preprocess_reorder(mut pipeline: Vec<Transform>) -> Vec<Transform> {
    // reorder Compose
    pipeline.sort_by(|a, b| match (a, b) {
        // don't reorder with From or Join or itself
        (
            Transform::From(_) | Transform::Join { .. } | Transform::Compute(_),
            Transform::From(_) | Transform::Join { .. } | Transform::Compute(_),
        ) => Ordering::Equal,

        // reorder always
        (Transform::Sort(_), Transform::Compute(_)) => Ordering::Greater,
        (Transform::Compute(_), Transform::Sort(_)) => Ordering::Less,

        // reorder if col decl is plain
        (Transform::Take(_), Transform::Compute(decl))
            if infer_complexity(decl) == Complexity::Plain =>
        {
            Ordering::Greater
        }
        (Transform::Compute(decl), Transform::Take(_))
            if infer_complexity(decl) == Complexity::Plain =>
        {
            Ordering::Less
        }

        // don't reorder by default
        _ => Ordering::Equal,
    });

    pipeline
}
