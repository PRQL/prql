/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;

use crate::ast::{ast_fold::AstFold, ColumnSort};

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
pub trait IrFold: AstFold {
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        fold_transform(self, transform)
    }
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> Result<Vec<Transform>> {
        fold_transforms(self, transforms)
    }
    fn fold_column_sort(&mut self, column_sort: ColumnSort) -> Result<ColumnSort> {
        fold_column_sort(self, column_sort)
    }
    fn fold_column_sorts(&mut self, columns: Vec<ColumnSort>) -> Result<Vec<ColumnSort>> {
        columns
            .into_iter()
            .map(|c| self.fold_column_sort(c))
            .try_collect()
    }
    fn fold_join_filter(&mut self, f: JoinFilter) -> Result<JoinFilter> {
        fold_join_filter(self, f)
    }
    fn fold_query(&mut self, query: Query) -> Result<Query> {
        fold_query(self, query)
    }
}

pub fn fold_query<F: ?Sized + IrFold>(fold: &mut F, query: Query) -> Result<Query> {
    Ok(Query {
        def: query.def,
        main_pipeline: fold.fold_transforms(query.main_pipeline)?,
        tables: query
            .tables
            .into_iter()
            .map(|t| {
                Ok::<_, anyhow::Error>(Table {
                    id: t.id,
                    name: t.name,
                    pipeline: fold.fold_transforms(t.pipeline)?,
                })
            })
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

pub fn fold_column_sort<T: ?Sized + IrFold>(
    fold: &mut T,
    sort_column: ColumnSort,
) -> Result<ColumnSort> {
    Ok(ColumnSort {
        direction: sort_column.direction,
        column: fold.fold_expr(sort_column.column)?,
    })
}

pub fn fold_transform<T: ?Sized + IrFold>(
    fold: &mut T,
    mut transform: Transform,
) -> Result<Transform> {
    transform.kind = match transform.kind {
        TransformKind::From(table) => TransformKind::From(table),

        TransformKind::Derive(assigns) => TransformKind::Derive(fold.fold_exprs(assigns)?),
        TransformKind::Select(assigns) => TransformKind::Select(fold.fold_exprs(assigns)?),
        TransformKind::Aggregate { assigns, by } => TransformKind::Aggregate {
            assigns: fold.fold_exprs(assigns)?,
            by: fold.fold_exprs(by)?,
        },

        TransformKind::Filter(f) => TransformKind::Filter(Box::new(fold.fold_expr(*f)?)),
        TransformKind::Sort(items) => TransformKind::Sort(fold.fold_column_sorts(items)?),
        TransformKind::Join { side, with, filter } => TransformKind::Join {
            side,
            with,
            filter: fold.fold_join_filter(filter)?,
        },
        TransformKind::Group { by, pipeline } => TransformKind::Group {
            by: fold.fold_exprs(by)?,
            pipeline: fold.fold_transforms(pipeline)?,
        },
        TransformKind::Take { by, range, sort } => TransformKind::Take {
            range,
            by: fold.fold_exprs(by)?,
            sort: fold.fold_column_sorts(sort)?,
        },
        TransformKind::Unique => TransformKind::Unique,
        TransformKind::Window {
            kind,
            range,
            pipeline,
        } => TransformKind::Window {
            kind,
            range,
            pipeline,
        },
    };
    Ok(transform)
}

pub fn fold_join_filter<T: ?Sized + IrFold>(fold: &mut T, f: JoinFilter) -> Result<JoinFilter> {
    Ok(match f {
        JoinFilter::On(nodes) => JoinFilter::On(fold.fold_exprs(nodes)?),
        JoinFilter::Using(nodes) => JoinFilter::Using(fold.fold_exprs(nodes)?),
    })
}
