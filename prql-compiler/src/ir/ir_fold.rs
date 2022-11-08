/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;

use crate::ast::InterpolateItem;

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
    fn fold_table(&mut self, table: TableDef) -> Result<TableDef> {
        fold_table(self, table)
    }
    fn fold_table_expr(&mut self, table_expr: TableExpr) -> Result<TableExpr> {
        fold_table_expr(self, table_expr)
    }
    fn fold_table_ref(&mut self, table_ref: TableRef) -> Result<TableRef> {
        fold_table_ref(self, table_ref)
    }
    fn fold_query(&mut self, query: Query) -> Result<Query> {
        fold_query(self, query)
    }
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
    fn fold_expr_kind(&mut self, kind: ExprKind) -> Result<ExprKind> {
        fold_expr_kind(self, kind)
    }
    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        Ok(ColumnDef {
            id: cd.id,
            kind: match cd.kind {
                ColumnDefKind::Wildcard => ColumnDefKind::Wildcard,
                ColumnDefKind::ExternRef(name) => ColumnDefKind::ExternRef(name),
                ColumnDefKind::Expr { name, expr } => ColumnDefKind::Expr {
                    name,
                    expr: self.fold_expr(expr)?,
                },
            },
        })
    }
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(cid)
    }
}

pub fn fold_table<F: ?Sized + IrFold>(fold: &mut F, t: TableDef) -> Result<TableDef> {
    Ok(TableDef {
        id: t.id,
        name: t.name,
        expr: fold.fold_table_expr(t.expr)?,
    })
}

pub fn fold_table_expr<F: ?Sized + IrFold>(fold: &mut F, t: TableExpr) -> Result<TableExpr> {
    Ok(match t {
        TableExpr::ExternRef(table_ref, defs) => TableExpr::ExternRef(
            table_ref,
            defs.into_iter()
                .map(|d| fold.fold_column_def(d))
                .try_collect()?,
        ),
        TableExpr::Pipeline(transforms) => TableExpr::Pipeline(fold.fold_transforms(transforms)?),
    })
}

pub fn fold_table_ref<F: ?Sized + IrFold>(fold: &mut F, table_ref: TableRef) -> Result<TableRef> {
    Ok(TableRef {
        name: table_ref.name,
        source: table_ref.source,
        columns: table_ref
            .columns
            .into_iter()
            .map(|c| fold.fold_column_def(c))
            .try_collect()?,
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

fn fold_cids<F: ?Sized + IrFold>(fold: &mut F, cids: Vec<CId>) -> Result<Vec<CId>> {
    cids.into_iter().map(|i| fold.fold_cid(i)).try_collect()
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
        From(tid) => From(fold.fold_table_ref(tid)?),

        Compute(assigns) => Compute(fold.fold_column_def(assigns)?),
        Aggregate(ids) => Aggregate(fold_cids(fold, ids)?),

        Select(ids) => Select(fold_cids(fold, ids)?),
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
            with: fold.fold_table_ref(with)?,
            filter: fold.fold_expr(filter)?,
        },
        Unique => Unique,
    };
    Ok(transform)
}

pub fn fold_expr_kind<F: ?Sized + IrFold>(fold: &mut F, kind: ExprKind) -> Result<ExprKind> {
    Ok(match kind {
        ExprKind::ColumnRef(cid) => ExprKind::ColumnRef(fold.fold_cid(cid)?),

        ExprKind::Binary { left, op, right } => ExprKind::Binary {
            left: Box::new(fold.fold_expr(*left)?),
            op,
            right: Box::new(fold.fold_expr(*right)?),
        },
        ExprKind::Unary { op, expr } => ExprKind::Unary {
            op,
            expr: Box::new(fold.fold_expr(*expr)?),
        },
        ExprKind::Range(range) => ExprKind::Range(Range {
            start: fold_optional_box(fold, range.start)?,
            end: fold_optional_box(fold, range.end)?,
        }),

        ExprKind::SString(items) => ExprKind::SString(fold_interpolate_items(fold, items)?),
        ExprKind::FString(items) => ExprKind::FString(fold_interpolate_items(fold, items)?),

        ExprKind::Literal(_) => kind,
    })
}

/// Helper
pub fn fold_optional_box<F: ?Sized + IrFold>(
    fold: &mut F,
    opt: Option<Box<Expr>>,
) -> Result<Option<Box<Expr>>> {
    Ok(match opt {
        Some(e) => Some(Box::new(fold.fold_expr(*e)?)),
        None => None,
    })
}

pub fn fold_interpolate_items<T: ?Sized + IrFold>(
    fold: &mut T,
    items: Vec<InterpolateItem<Expr>>,
) -> Result<Vec<InterpolateItem<Expr>>> {
    items
        .into_iter()
        .map(|i| fold_interpolate_item(fold, i))
        .try_collect()
}

pub fn fold_interpolate_item<T: ?Sized + IrFold>(
    fold: &mut T,
    item: InterpolateItem<Expr>,
) -> Result<InterpolateItem<Expr>> {
    Ok(match item {
        InterpolateItem::String(string) => InterpolateItem::String(string),
        InterpolateItem::Expr(expr) => InterpolateItem::Expr(Box::new(fold.fold_expr(*expr)?)),
    })
}
