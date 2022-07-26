use anyhow::Result;

use crate::ast::ast_fold::*;
use crate::ast::*;

pub fn un_group(query: ResolvedQuery) -> Result<ResolvedQuery> {
    UnGrouper {}.fold_resolved_query(query)
}

/// Traverses AST and replaces transforms with nested pipelines with the pipeline
struct UnGrouper {}

impl AstFold for UnGrouper {
    fn fold_transform(&mut self, mut transform: Transform) -> Result<Transform> {
        transform.kind = match transform.kind {
            TransformKind::Group { pipeline, by } => {
                let mut transforms = Vec::with_capacity(pipeline.transforms.len());

                for t in pipeline.transforms {
                    // ungroup inner
                    let t = self.fold_transform(t)?;

                    // remove all sorts
                    if !matches!(t.kind, TransformKind::Sort(_)) {
                        transforms.push(t);
                    }
                }
                TransformKind::Group {
                    by,
                    pipeline: ResolvedQuery { transforms },
                }
            }
            t => t,
        };
        Ok(transform)
    }
}
