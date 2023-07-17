use anyhow::Result;
use itertools::Itertools;

use self::ast::fold_range;
use crate::ast::pl::*;

pub use ast::AstFold;

pub mod ast;

pub fn fold_window<F: ?Sized + AstFold>(fold: &mut F, window: WindowFrame) -> Result<WindowFrame> {
    Ok(WindowFrame {
        kind: window.kind,
        range: fold_range(fold, window.range)?,
    })
}

pub fn fold_column_sorts<F: ?Sized + AstFold>(
    fold: &mut F,
    sort: Vec<ColumnSort>,
) -> Result<Vec<ColumnSort>> {
    sort.into_iter()
        .map(|s| fold_column_sort(fold, s))
        .try_collect()
}

pub fn fold_column_sort<T: ?Sized + AstFold>(
    fold: &mut T,
    sort_column: ColumnSort,
) -> Result<ColumnSort> {
    Ok(ColumnSort {
        direction: sort_column.direction,
        column: Box::new(fold.fold_expr(*sort_column.column)?),
    })
}

pub fn fold_transform_call<T: ?Sized + AstFold>(
    fold: &mut T,
    t: TransformCall,
) -> Result<TransformCall> {
    Ok(TransformCall {
        kind: Box::new(fold_transform_kind(fold, *t.kind)?),
        input: Box::new(fold.fold_expr(*t.input)?),
        partition: fold.fold_exprs(t.partition)?,
        frame: fold.fold_window(t.frame)?,
        sort: fold_column_sorts(fold, t.sort)?,
    })
}

pub fn fold_transform_kind<T: ?Sized + AstFold>(
    fold: &mut T,
    t: TransformKind,
) -> Result<TransformKind> {
    use TransformKind::*;
    Ok(match t {
        Derive { assigns } => Derive {
            assigns: fold.fold_exprs(assigns)?,
        },
        Select { assigns } => Select {
            assigns: fold.fold_exprs(assigns)?,
        },
        Filter { filter } => Filter {
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Aggregate { assigns } => Aggregate {
            assigns: fold.fold_exprs(assigns)?,
        },
        Sort { by } => Sort {
            by: fold_column_sorts(fold, by)?,
        },
        Take { range } => Take {
            range: fold_range(fold, range)?,
        },
        Join { side, with, filter } => Join {
            side,
            with: Box::new(fold.fold_expr(*with)?),
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Append(bottom) => Append(Box::new(fold.fold_expr(*bottom)?)),
        Group { by, pipeline } => Group {
            by: fold.fold_exprs(by)?,
            pipeline: Box::new(fold.fold_expr(*pipeline)?),
        },
        Window {
            kind,
            range,
            pipeline,
        } => Window {
            kind,
            range: fold_range(fold, range)?,
            pipeline: Box::new(fold.fold_expr(*pipeline)?),
        },
        Loop(pipeline) => Loop(Box::new(fold.fold_expr(*pipeline)?)),
    })
}
