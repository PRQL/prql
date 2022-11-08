use super::{IrFold, Transform};

/// Folder that counts the number of table referenced in a PRQL query.
#[derive(Debug, Default)]
pub struct TableCounter {
    count: usize,
}

impl TableCounter {
    pub fn count(&self) -> usize {
        self.count
    }
}

impl IrFold for TableCounter {
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> anyhow::Result<Vec<Transform>> {
        for transform in &transforms {
            if let Transform::Join { .. } | Transform::From(_) = transform {
                self.count += 1;
            }
        }

        Ok(transforms)
    }
}
