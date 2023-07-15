use anyhow::Result;
use itertools::Itertools;

use prql_ast::fold::{fold_range, AstFold, Fold};

use super::{
    ColumnSort, ExprKindExtra, FuncExtra, TransformCall, TransformKind, Ty, WindowFrame, X,
};

impl<F: AstFoldExtra> Fold<X, F> for ExprKindExtra {
    fn fold(self, folder: &mut F) -> Result<Self> {
        use ExprKindExtra::*;
        Ok(match self {
            All { within, except } => All {
                within,
                except: folder.fold_exprs(except)?,
            },

            TransformCall(transform) => TransformCall(folder.fold_transform_call(transform)?),
            RqOperator { name, args } => RqOperator {
                name,
                args: folder.fold_exprs(args)?,
            },
            // None of these capture variables, so we don't need to fold them.
            Type(_) => self,
        })
    }
}

impl<F: AstFoldExtra> Fold<X, F> for FuncExtra {
    fn fold(self, folder: &mut F) -> Result<Self> {
        Ok(FuncExtra {
            args: self
                .args
                .into_iter()
                .map(|item| folder.fold_expr(item))
                .try_collect()?,
            ..self
        })
    }
}

pub trait AstFoldExtra: AstFold<X> {
    fn fold_transform_call(&mut self, transform_call: TransformCall) -> Result<TransformCall> {
        fold_transform_call(self, transform_call)
    }

    fn fold_window(&mut self, window: WindowFrame) -> Result<WindowFrame> {
        fold_window(self, window)
    }

    fn fold_type(&mut self, t: Ty) -> Result<Ty> {
        Ok(t)
    }
}

pub fn fold_window<F: ?Sized + AstFoldExtra>(
    fold: &mut F,
    window: WindowFrame,
) -> Result<WindowFrame> {
    Ok(WindowFrame {
        kind: window.kind,
        range: fold_range(fold, window.range)?,
    })
}

pub fn fold_column_sorts<F: ?Sized + AstFoldExtra>(
    fold: &mut F,
    sort: Vec<ColumnSort>,
) -> Result<Vec<ColumnSort>> {
    sort.into_iter()
        .map(|s| fold_column_sort(fold, s))
        .try_collect()
}

pub fn fold_column_sort<F: ?Sized + AstFoldExtra>(
    fold: &mut F,
    sort_column: ColumnSort,
) -> Result<ColumnSort> {
    Ok(ColumnSort {
        direction: sort_column.direction,
        column: Box::new(fold.fold_expr(*sort_column.column)?),
    })
}

pub fn fold_transform_call<F: ?Sized + AstFold<X> + AstFoldExtra>(
    fold: &mut F,
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

pub fn fold_transform_kind<F: ?Sized + AstFoldExtra>(
    fold: &mut F,
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
