use std::collections::HashSet;

use super::{IrFold, TId, Transform};

/// Folder that counts the number of table referenced in a PRQL query.
#[derive(Debug, Default)]
pub struct TableCounter {
    tables: HashSet<TId>,
}

impl TableCounter {
    pub fn count(&self) -> usize {
        self.tables.len()
    }
}

impl IrFold for TableCounter {
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> anyhow::Result<Vec<Transform>> {
        for transform in &transforms {
            if let Transform::Join { with: tid, .. } | Transform::From(tid) = transform {
                self.tables.insert(*tid);
            }
        }

        Ok(transforms)
    }
}
