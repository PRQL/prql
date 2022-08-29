use anyhow::Result;

use crate::ast::ast_fold::*;
use crate::ast::*;

pub fn un_group(transforms: Vec<Transform>) -> Result<Vec<Transform>> {
    UnGrouper {}.fold_transforms(transforms)
}

/// Traverses AST and replaces transforms with nested pipelines with the pipeline
struct UnGrouper {}

impl AstFold for UnGrouper {
    fn fold_transform(&mut self, mut transform: Transform) -> Result<Transform> {
        transform.kind = match transform.kind {
            TransformKind::Group { pipeline, by } => {
                let mut res = Vec::with_capacity(pipeline.len());

                for t in pipeline {
                    // ungroup inner
                    let t = self.fold_transform(t)?;

                    // remove all sorts
                    if !matches!(t.kind, TransformKind::Sort(_)) {
                        res.push(t);
                    }
                }
                TransformKind::Group { by, pipeline: res }
            }
            t => t,
        };
        Ok(transform)
    }
}
