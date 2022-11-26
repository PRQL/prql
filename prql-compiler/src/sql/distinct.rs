use anyhow::Result;

use crate::ast::{BinOp, ColumnSort, InterpolateItem, Literal, Range, WindowFrame, WindowKind};
use crate::ir::{CId, ColumnDefKind, Expr, ExprKind, IrFold, Take, Transform, Window};

use super::context::AnchorContext;
use super::translator::Context;

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

impl<'a> IrFold for TakeConverter<'a> {
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
                        .map_err(|_| anyhow::anyhow!("Invaid take arguments"))?;

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
        let def = ColumnDefKind::Expr {
            name: Some(self.context.gen_column_name()),
            expr: Expr {
                kind: ExprKind::SString(vec![InterpolateItem::String("ROW_NUMBER()".to_string())]),
                span: None,
            },
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

        let def = self.context.register_column(def, Some(window), None);

        let col_ref = Box::new(Expr {
            kind: ExprKind::ColumnRef(def.id),
            span: None,
        });

        // add the two transforms
        let range_int = range.clone().try_map(as_int).unwrap();
        vec![
            Transform::Compute(def),
            Transform::Filter(match (range_int.start, range_int.end) {
                (Some(s), Some(e)) if s == e => Expr {
                    span: None,
                    kind: ExprKind::Binary {
                        left: col_ref,
                        op: BinOp::Eq,
                        right: int_expr(s),
                    },
                },
                (Some(s), None) => Expr {
                    span: None,
                    kind: ExprKind::Binary {
                        left: col_ref,
                        op: BinOp::Gte,
                        right: int_expr(s),
                    },
                },
                (None, Some(e)) => Expr {
                    span: None,
                    kind: ExprKind::Binary {
                        left: col_ref,
                        op: BinOp::Lte,
                        right: int_expr(e),
                    },
                },
                (Some(_), Some(_)) => Expr {
                    kind: ExprKind::SString(vec![
                        InterpolateItem::Expr(col_ref),
                        InterpolateItem::String(" BETWEEN ".to_string()),
                        InterpolateItem::Expr(Box::new(Expr {
                            kind: ExprKind::Range(range.map(Box::new)),
                            span: None,
                        })),
                    ]),
                    span: None,
                },
                (None, None) => Expr {
                    kind: ExprKind::Literal(Literal::Boolean(true)),
                    span: None,
                },
            }),
        ]
    }
}
