use anyhow::Result;

use crate::ast::ast_fold::*;
use crate::ast::*;

pub fn un_group(nodes: Vec<Node>) -> Result<Vec<Node>> {
    UnGrouper {}.fold_nodes(nodes)
}

/// Traverses AST and replaces transforms with nested pipelines with the pipeline
struct UnGrouper {}

impl AstFold for UnGrouper {
    fn fold_nodes(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        let mut res = Vec::new();

        for node in nodes {
            match node.item {
                Item::Transform(Transform::Group { pipeline, .. }) => {
                    let pipeline = self.fold_nodes(pipeline.item.into_pipeline().unwrap().nodes)?;

                    res.extend(pipeline.into_iter().filter(|x| {
                        // remove all sorts
                        x.item
                            .as_transform()
                            .map(|t| !matches!(t, Transform::Sort(_)))
                            .unwrap_or(true)
                    }));
                }
                _ => {
                    res.push(self.fold_node(node)?);
                }
            }
        }
        Ok(res)
    }
}
