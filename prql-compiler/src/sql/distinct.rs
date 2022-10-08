use anyhow::Result;

use crate::ast::{ast_fold::AstFold, *};
use crate::semantic::Declaration;

use super::materializer::MaterializationContext;

pub fn take_to_distinct(
    nodes: Vec<Node>,
    context: &mut MaterializationContext,
) -> Result<Vec<Node>> {
    let mut d = DistinctMaker { context };
    d.fold_nodes(nodes)
}
/// Creates [Transform::Unique] from [Transform::Take]
struct DistinctMaker<'a> {
    context: &'a mut MaterializationContext,
}

impl<'a> AstFold for DistinctMaker<'a> {
    fn fold_nodes(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        let mut res = Vec::new();

        for node in nodes {
            match node.item {
                Item::Transform(Transform::Take { ref by, .. }) if by.is_empty() => {
                    res.push(node);
                }

                Item::Transform(Transform::Take { range, by, sort }) => {
                    let range_int = range.clone().into_int()?;

                    let take_only_first =
                        range_int.start.unwrap_or(1) == 1 && matches!(range_int.end, Some(1));
                    if take_only_first && sort.is_empty() {
                        // TODO: use distinct only if `by == all columns in frame`
                        res.push(Item::Transform(Transform::Unique).into());
                        continue;
                    }

                    // convert `take range` into:
                    //   derive _rn = s"ROW NUMBER"
                    //   filter (_rn | in range)
                    res.extend(self.filter_row_number(range, sort, by));
                }
                _ => {
                    res.push(node);
                }
            }
        }
        Ok(res)
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
    ) -> Vec<Node> {
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
            Transform::Derive(vec![ident.clone()]),
            Transform::Filter(Box::new(match (range_int.start, range_int.end) {
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
        ];
        transforms
            .into_iter()
            .map(|t| Node {
                is_complex: true, // this transform DOES contain windowed functions
                ..Node::from(Item::Transform(t))
            })
            .collect()
    }
}
