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
    fn fold_table_expr(&mut self, table_expr: TableExpr) -> Result<TableExpr> {
        fold_table_expr(self, table_expr)
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
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(cid)
    }
}

pub fn fold_table<F: ?Sized + IrFold>(fold: &mut F, t: Table) -> Result<Table> {
    Ok(Table {
        id: t.id,
        name: t.name,
        expr: fold.fold_table_expr(t.expr)?,
    })
}

pub fn fold_table_expr<F: ?Sized + IrFold>(fold: &mut F, t: TableExpr) -> Result<TableExpr> {
    Ok(match t {
        TableExpr::Ref(r) => TableExpr::Ref(r),
        TableExpr::Pipeline(transforms) => TableExpr::Pipeline(fold.fold_transforms(transforms)?),
    })
}

pub fn fold_query<F: ?Sized + IrFold>(fold: &mut F, query: Query) -> Result<Query> {
    Ok(Query {
        def: query.def,
        expr: fold.fold_table_expr(query.expr)?,
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
    use Transform::*;

    transform = match transform {
        From(tid) => From(tid),

        Derive(assigns) => Derive(fold.fold_column_def(assigns)?),
        Aggregate(column_defs) => Aggregate(
            column_defs
                .into_iter()
                .map(|cd| fold.fold_column_def(cd))
                .try_collect()?,
        ),

        Select(ids) => Select(ids.into_iter().map(|i| fold.fold_cid(i)).try_collect()?),
        Filter(i) => Filter(i),
        Sort(sorts) => Sort(
            sorts
                .into_iter()
                .map(|s| -> Result<ColumnSort<CId>> {
                    Ok(ColumnSort {
                        column: fold.fold_cid(s.column)?,
                        direction: s.direction,
                    })
                })
                .try_collect()?,
        ),
        Take(range) => Take(range),
        Join { side, with, filter } => Join {
            side,
            with,
            filter: match filter {
                JoinFilter::On(ids) => {
                    JoinFilter::On(ids.into_iter().map(|i| fold.fold_cid(i)).try_collect()?)
                }
                JoinFilter::Using(ids) => {
                    JoinFilter::Using(ids.into_iter().map(|i| fold.fold_cid(i)).try_collect()?)
                }
            },
        },
        Unique => Unique,
    };
    Ok(transform)
}
