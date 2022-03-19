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
    fn fold_pipeline(&mut self, pipeline: Pipeline) -> Result<Pipeline> {
        pipeline
            .into_iter()
            .map(|t| self.fold_transformation(t))
            .collect()
    }
    fn fold_ident(&mut self, ident: Ident) -> Result<Ident> {
        Ok(ident)
    }
    fn fold_items(&mut self, items: Items) -> Result<Items> {
        items.into_iter().map(|item| self.fold_item(item)).collect()
    }
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        Ok(Table {
            name: self.fold_ident(table.name)?,
            pipeline: self.fold_pipeline(table.pipeline)?,
        })
    }
    fn fold_named_arg(&mut self, named_arg: NamedArg) -> Result<NamedArg> {
        Ok(NamedArg {
            name: self.fold_ident(named_arg.name)?,
            arg: Box::new(self.fold_item(*named_arg.arg)?),
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
    fn fold_terms(&mut self, terms: Items) -> Result<Items> {
        fold_terms(self, terms)
    }
    fn fold_transformation(&mut self, transformation: Transformation) -> Result<Transformation> {
        fold_transformation(self, transformation)
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        fold_item(self, item)
    }
    fn fold_function(&mut self, function: Function) -> Result<Function> {
        fold_function(self, function)
    }
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_call)
    }
    fn fold_assign(&mut self, assign: Assign) -> Result<Assign> {
        fold_assign(self, assign)
    }
    fn fold_sstring_item(&mut self, sstring_item: SStringItem) -> Result<SStringItem> {
        fold_sstring_item(self, sstring_item)
    }
    fn fold_inline_pipeline(&mut self, inline_pipeline: Items) -> Result<Items> {
        fold_inline_pipeline(self, inline_pipeline)
    }
}
pub fn fold_terms<T: ?Sized + AstFold>(fold: &mut T, terms: Items) -> Result<Items> {
    terms.into_iter().map(|item| fold.fold_item(item)).collect()
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
        Transformation::Derive(assigns) => Ok(Transformation::Derive({
            assigns
                .into_iter()
                .map(|assign| fold.fold_assign(assign))
                .try_collect()?
        })),
        Transformation::From(ident) => Ok(Transformation::From(fold.fold_ident(ident)?)),
        Transformation::Filter(Filter(items)) => {
            Ok(Transformation::Filter(Filter(fold.fold_items(items)?)))
        }
        Transformation::Sort(items) => Ok(Transformation::Sort(fold.fold_items(items)?)),
        Transformation::Join { side, with, on } => Ok(Transformation::Join {
            side,
            with,
            on: fold.fold_items(on)?,
        }),
        Transformation::Select(items) => Ok(Transformation::Select(fold.fold_items(items)?)),
        Transformation::Aggregate { by, calcs, assigns } => Ok(Transformation::Aggregate {
            by: fold.fold_items(by)?,
            calcs: fold.fold_items(calcs)?,
            assigns: assigns
                .into_iter()
                .map(|assign| fold.fold_assign(assign))
                .try_collect()?,
        }),
        Transformation::Func(func_call) => {
            Ok(Transformation::Func(fold.fold_func_call(func_call)?))
        }
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
            .map(|named_arg| fold.fold_named_arg(named_arg))
            .try_collect()?,
    })
}
pub fn fold_item<T: ?Sized + AstFold>(fold: &mut T, item: Item) -> Result<Item> {
    Ok(match item {
        Item::Ident(ident) => Item::Ident(fold.fold_ident(ident)?),
        Item::Terms(terms) => Item::Terms(fold.fold_terms(terms)?),
        Item::Expr(items) => Item::Expr(fold.fold_items(items)?),
        Item::Idents(idents) => Item::Idents(
            idents
                .into_iter()
                .map(|i| fold.fold_ident(i))
                .try_collect()?,
        ),
        // We could implement a `fold_list_item`...
        Item::List(items) => Item::List(
            items
                .into_iter()
                .map(|list_item| {
                    list_item
                        .into_inner()
                        .into_iter()
                        .map(|item| fold.fold_item(item))
                        .try_collect()
                        .map(ListItem)
                })
                .try_collect()?,
        ),
        Item::Query(Query { items }) => Item::Query(Query {
            items: fold.fold_items(items)?,
        }),
        Item::InlinePipeline(items) => Item::InlinePipeline(fold.fold_inline_pipeline(items)?),
        Item::Pipeline(transformations) => Item::Pipeline(
            transformations
                .into_iter()
                .map(|t| fold.fold_transformation(t))
                .try_collect()?,
        ),
        Item::NamedArg(named_arg) => Item::NamedArg(fold.fold_named_arg(named_arg)?),
        Item::Assign(assign) => Item::Assign(fold.fold_assign(assign)?),
        Item::Transformation(transformation) => {
            Item::Transformation(fold.fold_transformation(transformation)?)
        }
        Item::SString(items) => Item::SString(
            items
                .into_iter()
                .map(|x| fold.fold_sstring_item(x))
                .try_collect()?,
        ),
        Item::Function(func) => Item::Function(fold.fold_function(func)?),
        // TODO: implement for these
        Item::Table(_) => item,
        // None of these capture variables, so we don't need to replace
        // them.
        Item::String(_) | Item::Raw(_) | Item::Todo(_) => item,
    })
}
pub fn fold_function<T: ?Sized + AstFold>(fold: &mut T, function: Function) -> Result<Function> {
    Ok(Function {
        name: fold.fold_ident(function.name)?,
        params: function
            .params
            .into_iter()
            .map(|param| match param {
                FunctionParam::Required(ident) => {
                    fold.fold_ident(ident).map(FunctionParam::Required)
                }
                FunctionParam::Named(named) => fold.fold_named_arg(named).map(FunctionParam::Named),
            })
            .try_collect()?,
        body: fold.fold_items(function.body)?,
    })
}
pub fn fold_assign<T: ?Sized + AstFold>(fold: &mut T, assign: Assign) -> Result<Assign> {
    Ok(Assign {
        lvalue: fold.fold_ident(assign.lvalue)?,
        rvalue: Box::new(fold.fold_item(*assign.rvalue)?),
    })
}
pub fn fold_inline_pipeline<T: ?Sized + AstFold>(
    fold: &mut T,
    inline_pipeline: Items,
) -> Result<Items> {
    fold.fold_items(inline_pipeline)
}
