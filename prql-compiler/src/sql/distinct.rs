use anyhow::Result;

use crate::ast::{ast_fold::AstFold, *};
use crate::semantic::Declaration;

use super::materializer::MaterializationContext;

pub fn take_to_distinct(
    query: ResolvedQuery,
    context: &mut MaterializationContext,
) -> Result<ResolvedQuery> {
    let mut d = DistinctMaker { context };
    d.fold_resolved_query(query)
}
/// Creates [Transform::Unique] from [Transform::Take]
struct DistinctMaker<'a> {
    context: &'a mut MaterializationContext,
}

impl<'a> AstFold for DistinctMaker<'a> {
    fn fold_resolved_query(&mut self, query: ResolvedQuery) -> Result<ResolvedQuery> {
        let mut res = Vec::new();

        for transform in query.transforms {
            match transform.kind {
                TransformKind::Take { ref by, .. } if by.is_empty() => {
                    res.push(transform);
                }

                TransformKind::Take { range, by, sort } => {
                    let range_int = range.clone().into_int()?;

                    let take_only_first =
                        range_int.start.unwrap_or(1) == 1 && matches!(range_int.end, Some(1));
                    if take_only_first && sort.is_empty() {
                        // TODO: use distinct only if `by == all columns in frame`
                        res.push(TransformKind::Unique.into());
                        continue;
                    }

                    // convert `take range` into:
                    //   derive _rn = s"ROW NUMBER"
                    //   filter (_rn | in range)
                    res.extend(self.filter_row_number(range, sort, by));
                }
                _ => {
                    res.push(transform);
                }
            }
        }
        Ok(ResolvedQuery { transforms: res })
    }

    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        Ok(function)
    }
}

impl<'a> DistinctMaker<'a> {
    fn filter_row_number(
        &mut self,
        range: Range,
        sort: Vec<ColumnSort>,
        by: Vec<Node>,
    ) -> Vec<Transform> {
        let range_int = range.clone().into_int().unwrap();

        // declare new column
        let decl = Node::from(Item::SString(vec![InterpolateItem::String(
            "ROW_NUMBER()".to_string(),
        )]));
        let is_unsorted = sort.is_empty();
        let windowed = Windowed {
            expr: Box::new(decl),
            group: by,
            sort,
            window: if is_unsorted {
                (WindowKind::Rows, Range::unbounded())
            } else {
                (WindowKind::Range, Range::from_ints(None, Some(0)))
            },
        };
        let decl = Declaration::Expression(Box::new(Item::Windowed(windowed).into()));
        let row_number_id = self.context.declarations.push(decl, None);

        // name it _rn_X where X is the row_number_id
        let mut ident = Node::from(Item::Ident(format!("_rn_{}", row_number_id)));
        ident.declared_at = Some(row_number_id);

        // add the two transforms
        let transforms = vec![
            TransformKind::Derive(vec![ident.clone()]).into(),
            Transform {
                kind: TransformKind::Filter(Box::new(match (range_int.start, range_int.end) {
                    (Some(s), Some(e)) if s == e => Node::from(Item::Binary {
                        left: Box::new(ident),
                        op: BinOp::Eq,
                        right: Box::new(Item::Literal(Literal::Integer(s)).into()),
                    }),
                    (Some(s), None) => Node::from(Item::Binary {
                        left: Box::new(ident),
                        op: BinOp::Gte,
                        right: Box::new(Item::Literal(Literal::Integer(s)).into()),
                    }),
                    (None, Some(e)) => Node::from(Item::Binary {
                        left: Box::new(ident),
                        op: BinOp::Lte,
                        right: Box::new(Item::Literal(Literal::Integer(e)).into()),
                    }),
                    (Some(_), Some(_)) => Item::SString(vec![
                        InterpolateItem::Expr(Box::new(ident)),
                        InterpolateItem::String(" BETWEEN ".to_string()),
                        InterpolateItem::Expr(Box::new(Item::Range(range).into())),
                    ])
                    .into(),
                    (None, None) => Item::Literal(Literal::Boolean(true)).into(),
                })),
                is_complex: true, // this transform DOES contain windowed functions
                ty: Frame::default(),
            },
        ];

        transforms
    }
}
