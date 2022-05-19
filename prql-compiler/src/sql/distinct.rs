use anyhow::{bail, Result};

use crate::{
    ast::{ast_fold::AstFold, *},
    error::{Error, Reason},
};

pub fn take_to_distinct(nodes: Vec<Node>) -> Result<Vec<Node>> {
    let mut d = DistinctMaker {};
    d.fold_nodes(nodes)
}
/// Creates [Transform::Unique] from [Transform::Take]
struct DistinctMaker {}

impl AstFold for DistinctMaker {
    fn fold_nodes(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        let mut res = Vec::new();

        for node in nodes {
            match node.item {
                Item::Transform(Transform::Take { ref by, .. }) if by.is_empty() => {
                    res.push(node);
                }

                Item::Transform(Transform::Take { range, .. }) => {
                    let range = range.into_int()?;
                    let take_only_first =
                        range.start.unwrap_or(1) == 1 && matches!(range.end, Some(1));

                    if take_only_first {
                        // TODO: use distinct only if `by == all columns in frame`
                        res.push(Item::Transform(Transform::Unique).into());
                    } else {
                        bail!(Error::new(Reason::Simple(
                            "`take` within `group` currently only supports argument `1`".to_string(),
                        ))
                        .with_span(node.span)
                        .with_help("For now, you can derive a row number within a group and do a filter on that."));
                    }

                    // TODO: else
                    //  return `
                    //    derive _rn = row number over (order by)
                    //    filter (_rn | in range)
                    //  `
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
