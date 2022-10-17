/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;

use super::*;

// Fold pattern:
// - https://rust-unofficial.github.io/patterns/patterns/creational/fold.html
// Good discussions on the visitor / fold pattern:
// - https://github.com/rust-unofficial/patterns/discussions/236 (within this,
//   this comment looked interesting: https://github.com/rust-unofficial/patterns/discussions/236#discussioncomment-393517)
// - https://news.ycombinator.com/item?id=25620110

// For some functions, we want to call a default impl, because copying &
// pasting everything apart from a specific match is lots of repetition. So
// we define a function outside the trait, by default call it, and let
// implementors override the default while calling the function directly for
// some cases. Ref https://stackoverflow.com/a/66077767/3064736
pub trait IrFold {
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        fold_transform(self, transform)
    }
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> Result<Vec<Transform>> {
        fold_transforms(self, transforms)
    }
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        fold_table(self, table)
    }
    fn fold_query(&mut self, query: Query) -> Result<Query> {
        fold_query(self, query)
    }
    fn fold_ir_expr(&mut self, expr: Expr) -> Result<Expr> {
        Ok(expr) // TODO: actually fold this when needed
    }
    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        Ok(ColumnDef {
            id: cd.id,
            name: cd.name,
            expr: self.fold_ir_expr(cd.expr)?,
        })
    }
}

pub fn fold_table<F: ?Sized + IrFold>(fold: &mut F, t: Table) -> Result<Table> {
    Ok(Table {
        id: t.id,
        name: t.name,
        pipeline: fold.fold_transforms(t.pipeline)?,
    })
}

pub fn fold_query<F: ?Sized + IrFold>(fold: &mut F, query: Query) -> Result<Query> {
    Ok(Query {
        def: query.def,
        main_pipeline: fold.fold_transforms(query.main_pipeline)?,
        tables: query
            .tables
            .into_iter()
            .map(|t| fold.fold_table(t))
            .try_collect()?,
    })
}

pub fn fold_transforms<F: ?Sized + IrFold>(
    fold: &mut F,
    transforms: Vec<Transform>,
) -> Result<Vec<Transform>> {
    transforms
        .into_iter()
        .map(|t| fold.fold_transform(t))
        .try_collect()
}

pub fn fold_transform<T: ?Sized + IrFold>(
    fold: &mut T,
    mut transform: Transform,
) -> Result<Transform> {
    transform = match transform {
        Transform::Derive(assigns) => Transform::Derive(fold.fold_column_def(assigns)?),
        Transform::Aggregate(column_defs) => Transform::Aggregate(
            column_defs
                .into_iter()
                .map(|cd| fold.fold_column_def(cd))
                .try_collect()?,
        ),

        kind => kind,
    };
    Ok(transform)
}
