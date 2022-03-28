/// A trait to "fold" a PRQL AST (similiar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use super::ast::*;
use anyhow::Result;
use itertools::Itertools;

// Fold pattern:
// - https://rust-unofficial.github.io/patterns/patterns/creational/fold.html
// Good discussions on the visitor / fold pattern:
// - https://github.com/rust-unofficial/patterns/discussions/236 (within this,
//   this comment looked interesting: https://github.com/rust-unofficial/patterns/discussions/236#discussioncomment-393517)
// - https://news.ycombinator.com/item?id=25620110

// TODO: some of these impls will be too specific because they were copied from
// when ReplaceVariables was implemented directly. When we find a case that is
// overfit on ReplaceVariables, we should add the custom impl to
// ReplaceVariables, and write a more generic impl to this.
pub trait AstFold {
    fn fold_pipeline(&mut self, pipeline: Vec<Transformation>) -> Result<Vec<Transformation>> {
        pipeline
            .into_iter()
            .map(|t| self.fold_transformation(t))
            .collect()
    }
    fn fold_ident(&mut self, ident: Ident) -> Result<Ident> {
        Ok(ident)
    }
    fn fold_items(&mut self, items: Vec<Item>) -> Result<Vec<Item>> {
        items.into_iter().map(|item| self.fold_item(item)).collect()
    }
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        Ok(Table {
            name: self.fold_ident(table.name)?,
            pipeline: self.fold_pipeline(table.pipeline)?,
        })
    }
    fn fold_filter(&mut self, filter: Filter) -> Result<Filter> {
        Ok(Filter(
            filter
                .0
                .into_iter()
                .map(|i| self.fold_item(i))
                .try_collect()?,
        ))
    }
    // For some functions, we want to call a default impl, because copying &
    // pasting everything apart from a specific match is lots of repetition. So
    // we define a function outside the trait, by default call it, and let
    // implementors override the default while calling the function directly for
    // some cases. Feel free to extend the functions that are separate when
    // necessary. Ref https://stackoverflow.com/a/66077767/3064736
    fn fold_transformation(&mut self, transformation: Transformation) -> Result<Transformation> {
        fold_transformation(self, transformation)
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        fold_item(self, item)
    }
    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        fold_func_def(self, function)
    }
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<Item> {
        Ok(Item::FuncCall(fold_func_call(self, func_call)?))
    }
    fn fold_func_curry(&mut self, func_curry: FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_curry)
    }
    fn fold_table_ref(&mut self, table_ref: TableRef) -> Result<TableRef> {
        fold_table_ref(self, table_ref)
    }
    fn fold_named_expr(&mut self, named_expr: NamedExpr) -> Result<NamedExpr> {
        fold_named_expr(self, named_expr)
    }
    fn fold_sstring_item(&mut self, sstring_item: SStringItem) -> Result<SStringItem> {
        fold_sstring_item(self, sstring_item)
    }
}
pub fn fold_sstring_item<T: ?Sized + AstFold>(
    fold: &mut T,
    sstring_item: SStringItem,
) -> Result<SStringItem> {
    Ok(match sstring_item {
        SStringItem::String(string) => SStringItem::String(string),
        SStringItem::Expr(expr) => SStringItem::Expr(fold.fold_item(expr)?),
    })
}
pub fn fold_transformation<T: ?Sized + AstFold>(
    fold: &mut T,
    transformation: Transformation,
) -> Result<Transformation> {
    match transformation {
        Transformation::Derive(assigns) => Ok(Transformation::Derive(fold.fold_items(assigns)?)),
        Transformation::From(table) => Ok(Transformation::From(fold.fold_table_ref(table)?)),
        Transformation::Filter(Filter(items)) => {
            Ok(Transformation::Filter(Filter(fold.fold_items(items)?)))
        }
        Transformation::Sort(items) => Ok(Transformation::Sort(fold.fold_items(items)?)),
        Transformation::Join { side, with, on } => Ok(Transformation::Join {
            side,
            with: fold.fold_table_ref(with)?,
            on: fold.fold_items(on)?,
        }),
        Transformation::Select(items) => Ok(Transformation::Select(fold.fold_items(items)?)),
        Transformation::Aggregate { by, select } => Ok(Transformation::Aggregate {
            by: fold.fold_items(by)?,
            select: fold.fold_items(select)?,
        }),
        // TODO: generalize? Or this never changes?
        Transformation::Take(_) => Ok(transformation),
    }
}

pub fn fold_func_call<T: ?Sized + AstFold>(fold: &mut T, func_call: FuncCall) -> Result<FuncCall> {
    Ok(FuncCall {
        // TODO: generalize? Or this never changes?
        name: func_call.name.to_owned(),
        args: func_call
            .args
            .into_iter()
            .map(|item| fold.fold_item(item))
            .try_collect()?,
        named_args: func_call
            .named_args
            .into_iter()
            .map(|arg| fold.fold_named_expr(arg))
            .try_collect()?,
    })
}

pub fn fold_table_ref<T: ?Sized + AstFold>(fold: &mut T, table: TableRef) -> Result<TableRef> {
    Ok(TableRef {
        name: fold.fold_ident(table.name)?,
        alias: table.alias.map(|a| fold.fold_ident(a)).transpose()?,
    })
}

pub fn fold_item<T: ?Sized + AstFold>(fold: &mut T, item: Item) -> Result<Item> {
    Ok(match item {
        Item::Ident(ident) => Item::Ident(fold.fold_ident(ident)?),
        Item::Expr(items) => Item::Expr(fold.fold_items(items)?),
        Item::List(items) => Item::List(
            items
                .into_iter()
                .map(|x| fold.fold_item(x.into_inner()).map(ListItem))
                .try_collect()?,
        ),
        Item::Query(Query { items }) => Item::Query(Query {
            items: fold.fold_items(items)?,
        }),
        Item::InlinePipeline(InlinePipeline { value, functions }) => {
            Item::InlinePipeline(InlinePipeline {
                value: Box::from(fold.fold_item(*value)?),
                functions: functions
                    .into_iter()
                    .map(|x| fold.fold_func_curry(x))
                    .try_collect()?,
            })
        }
        Item::Pipeline(transformations) => Item::Pipeline(
            transformations
                .into_iter()
                .map(|t| fold.fold_transformation(t))
                .try_collect()?,
        ),
        Item::NamedExpr(named_expr) => Item::NamedExpr(fold.fold_named_expr(named_expr)?),
        Item::Transformation(transformation) => {
            Item::Transformation(fold.fold_transformation(transformation)?)
        }
        Item::SString(items) => Item::SString(
            items
                .into_iter()
                .map(|x| fold.fold_sstring_item(x))
                .try_collect()?,
        ),
        Item::FuncDef(func) => Item::FuncDef(fold.fold_func_def(func)?),
        Item::FuncCall(func_call) => fold.fold_func_call(func_call)?,
        Item::Table(table) => Item::Table(Table {
            name: table.name,
            pipeline: fold.fold_pipeline(table.pipeline)?,
        }),
        // None of these capture variables, so we don't need to replace
        // them.
        Item::String(_) | Item::Raw(_) | Item::Todo(_) => item,
    })
}
pub fn fold_func_def<T: ?Sized + AstFold>(fold: &mut T, function: FuncDef) -> Result<FuncDef> {
    Ok(FuncDef {
        name: fold.fold_ident(function.name)?,
        positional_params: function
            .positional_params
            .into_iter()
            .map(|ident| fold.fold_ident(ident))
            .try_collect()?,
        named_params: function
            .named_params
            .into_iter()
            .map(|named_param| fold.fold_named_expr(named_param))
            .try_collect()?,
        body: Box::new(fold.fold_item(*function.body)?),
    })
}
pub fn fold_named_expr<T: ?Sized + AstFold>(
    fold: &mut T,
    named_expr: NamedExpr,
) -> Result<NamedExpr> {
    Ok(NamedExpr {
        name: fold.fold_ident(named_expr.name)?,
        expr: Box::new(fold.fold_item(*named_expr.expr)?),
    })
}
