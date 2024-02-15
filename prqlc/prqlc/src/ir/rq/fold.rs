/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use itertools::Itertools;

use crate::ir::generic::{ColumnSort, WindowFrame};
use crate::Result;

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
pub trait RqFold {
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        fold_transform(self, transform)
    }
    fn fold_transforms(&mut self, transforms: Vec<Transform>) -> Result<Vec<Transform>> {
        fold_transforms(self, transforms)
    }
    fn fold_table(&mut self, table: TableDecl) -> Result<TableDecl> {
        fold_table(self, table)
    }
    fn fold_relation(&mut self, relation: Relation) -> Result<Relation> {
        fold_relation(self, relation)
    }
    fn fold_relation_kind(&mut self, rel_kind: RelationKind) -> Result<RelationKind> {
        fold_relation_kind(self, rel_kind)
    }
    fn fold_table_ref(&mut self, table_ref: TableRef) -> Result<TableRef> {
        fold_table_ref(self, table_ref)
    }
    fn fold_query(&mut self, query: RelationalQuery) -> Result<RelationalQuery> {
        fold_query(self, query)
    }
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
    fn fold_expr_kind(&mut self, kind: ExprKind) -> Result<ExprKind> {
        fold_expr_kind(self, kind)
    }
    fn fold_relation_column(&mut self, col: RelationColumn) -> Result<RelationColumn> {
        Ok(col)
    }
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(cid)
    }
    fn fold_cids(&mut self, cids: Vec<CId>) -> Result<Vec<CId>> {
        cids.into_iter().map(|i| self.fold_cid(i)).try_collect()
    }
    fn fold_compute(&mut self, compute: Compute) -> Result<Compute> {
        fold_compute(self, compute)
    }
}

fn fold_compute<F: ?Sized + RqFold>(
    fold: &mut F,
    compute: Compute,
) -> Result<Compute> {
    Ok(Compute {
        id: fold.fold_cid(compute.id)?,
        expr: fold.fold_expr(compute.expr)?,
        window: compute.window.map(|w| fold_window(fold, w)).transpose()?,
        is_aggregation: compute.is_aggregation,
    })
}

fn fold_window<F: ?Sized + RqFold>(fold: &mut F, w: Window) -> Result<Window> {
    Ok(Window {
        frame: WindowFrame {
            kind: w.frame.kind,
            range: Range {
                start: w.frame.range.start.map(|x| fold.fold_expr(x)).transpose()?,
                end: w.frame.range.end.map(|x| fold.fold_expr(x)).transpose()?,
            },
        },
        partition: fold.fold_cids(w.partition)?,
        sort: fold_column_sorts(fold, w.sort)?,
    })
}

pub fn fold_table<F: ?Sized + RqFold>(fold: &mut F, t: TableDecl) -> Result<TableDecl> {
    Ok(TableDecl {
        id: t.id,
        name: t.name,
        relation: fold.fold_relation(t.relation)?,
    })
}

pub fn fold_relation<F: ?Sized + RqFold>(
    fold: &mut F,
    relation: Relation,
) -> Result<Relation> {
    Ok(Relation {
        kind: fold.fold_relation_kind(relation.kind)?,
        columns: relation.columns,
    })
}

pub fn fold_relation_kind<F: ?Sized + RqFold>(
    fold: &mut F,
    rel: RelationKind,
) -> Result<RelationKind> {
    Ok(match rel {
        RelationKind::ExternRef(table_ref) => RelationKind::ExternRef(table_ref),
        RelationKind::Pipeline(transforms) => {
            RelationKind::Pipeline(fold.fold_transforms(transforms)?)
        }
        RelationKind::Literal(lit) => RelationKind::Literal(lit),
        RelationKind::SString(items) => RelationKind::SString(fold_interpolate_items(fold, items)?),
        RelationKind::BuiltInFunction { name, args } => RelationKind::BuiltInFunction {
            name,
            args: args.into_iter().map(|a| fold.fold_expr(a)).try_collect()?,
        },
    })
}

pub fn fold_table_ref<F: ?Sized + RqFold>(fold: &mut F, table_ref: TableRef) -> Result<TableRef> {
    Ok(TableRef {
        name: table_ref.name,
        source: table_ref.source,
        columns: table_ref
            .columns
            .into_iter()
            .map(|(col, cid)| -> Result<_> {
                Ok((fold.fold_relation_column(col)?, fold.fold_cid(cid)?))
            })
            .try_collect()?,
    })
}

pub fn fold_query<F: ?Sized + RqFold>(
    fold: &mut F,
    query: RelationalQuery,
) -> Result<RelationalQuery> {
    Ok(RelationalQuery {
        def: query.def,
        relation: fold.fold_relation(query.relation)?,
        tables: query
            .tables
            .into_iter()
            .map(|t| fold.fold_table(t))
            .try_collect()?,
    })
}

pub fn fold_transforms<F: ?Sized + RqFold>(
    fold: &mut F,
    transforms: Vec<Transform>,
) -> Result<Vec<Transform>> {
    transforms
        .into_iter()
        .map(|t| fold.fold_transform(t))
        .try_collect()
}

pub fn fold_transform<T: ?Sized + RqFold>(
    fold: &mut T,
    mut transform: Transform,
) -> Result<Transform> {
    use Transform::*;

    transform = match transform {
        From(tid) => From(fold.fold_table_ref(tid)?),

        Compute(compute) => Compute(fold.fold_compute(compute)?),
        Aggregate { partition, compute } => Aggregate {
            partition: fold.fold_cids(partition)?,
            compute: fold.fold_cids(compute)?,
        },
        Select(ids) => Select(fold.fold_cids(ids)?),
        Filter(i) => Filter(fold.fold_expr(i)?),
        Sort(sorts) => Sort(fold_column_sorts(fold, sorts)?),
        Take(take) => Take(super::Take {
            partition: fold.fold_cids(take.partition)?,
            sort: fold_column_sorts(fold, take.sort)?,
            range: take.range,
        }),
        Join { side, with, filter } => Join {
            side,
            with: fold.fold_table_ref(with)?,
            filter: fold.fold_expr(filter)?,
        },
        Append(bottom) => Append(fold.fold_table_ref(bottom)?),
        Loop(transforms) => Loop(fold_transforms(fold, transforms)?),
    };
    Ok(transform)
}

pub fn fold_column_sorts<T: ?Sized + RqFold>(
    fold: &mut T,
    sorts: Vec<ColumnSort<CId>>,
) -> Result<Vec<ColumnSort<CId>>> {
    sorts
        .into_iter()
        .map(|s| -> Result<ColumnSort<CId>> {
            Ok(ColumnSort {
                column: fold.fold_cid(s.column)?,
                direction: s.direction,
            })
        })
        .try_collect()
}

pub fn fold_expr_kind<F: ?Sized + RqFold>(fold: &mut F, kind: ExprKind) -> Result<ExprKind> {
    Ok(match kind {
        ExprKind::ColumnRef(cid) => ExprKind::ColumnRef(fold.fold_cid(cid)?),

        ExprKind::SString(items) => ExprKind::SString(fold_interpolate_items(fold, items)?),
        ExprKind::Case(cases) => ExprKind::Case(
            cases
                .into_iter()
                .map(|c| fold_switch_case(fold, c))
                .try_collect()?,
        ),
        ExprKind::Operator { name, args } => ExprKind::Operator {
            name,
            args: args.into_iter().map(|a| fold.fold_expr(a)).try_collect()?,
        },
        ExprKind::Param(id) => ExprKind::Param(id),

        ExprKind::Literal(_) => kind,
        ExprKind::Array(exprs) => {
            ExprKind::Array(exprs.into_iter().map(|e| fold.fold_expr(e)).try_collect()?)
        }
    })
}

/// Helper
pub fn fold_optional_box<F: ?Sized + RqFold>(
    fold: &mut F,
    opt: Option<Box<Expr>>,
) -> Result<Option<Box<Expr>>> {
    Ok(match opt {
        Some(e) => Some(Box::new(fold.fold_expr(*e)?)),
        None => None,
    })
}

pub fn fold_interpolate_items<T: ?Sized + RqFold>(
    fold: &mut T,
    items: Vec<InterpolateItem>,
) -> Result<Vec<InterpolateItem>> {
    items
        .into_iter()
        .map(|i| fold_interpolate_item(fold, i))
        .try_collect()
}

pub fn fold_interpolate_item<T: ?Sized + RqFold>(
    fold: &mut T,
    item: InterpolateItem,
) -> Result<InterpolateItem> {
    Ok(match item {
        InterpolateItem::String(string) => InterpolateItem::String(string),
        InterpolateItem::Expr { expr, format } => InterpolateItem::Expr {
            expr: Box::new(fold.fold_expr(*expr)?),
            format,
        },
    })
}

pub fn fold_switch_case<F: ?Sized + RqFold>(fold: &mut F, case: SwitchCase) -> Result<SwitchCase> {
    Ok(SwitchCase {
        condition: fold.fold_expr(case.condition)?,
        value: fold.fold_expr(case.value)?,
    })
}
