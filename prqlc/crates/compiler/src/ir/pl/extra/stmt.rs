use anyhow::{bail, Result};

use crate::ir::pl::{Annotation, ExprKind};

impl Annotation {
    /// Find the items in a `@{a=b}`. We're only using annotations with tuples;
    /// we can consider formalizing this constraint.
    pub fn tuple_items(self) -> Result<Vec<(String, ExprKind)>> {
        match self.expr.kind {
            ExprKind::Tuple(items) => items
                .into_iter()
                .map(|item| Ok((item.alias.clone().unwrap(), item.kind)))
                .collect(),
            _ => bail!("Annotation must be a tuple"),
        }
    }
}
