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
    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        node.item = self.fold_item(node.item)?;
        Ok(node)
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        fold_item(self, item)
    }
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        items.into_iter().map(|item| self.fold_node(item)).collect()
    }

    fn fold_pipeline(&mut self, pipeline: Vec<Transform>) -> Result<Vec<Transform>> {
        pipeline
            .into_iter()
            .map(|t| self.fold_transform(t))
            .collect()
    }
    fn fold_ident(&mut self, ident: Ident) -> Result<Ident> {
        Ok(ident)
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
                .map(|i| self.fold_node(i))
                .try_collect()?,
        ))
    }
    // For some functions, we want to call a default impl, because copying &
    // pasting everything apart from a specific match is lots of repetition. So
    // we define a function outside the trait, by default call it, and let
    // implementors override the default while calling the function directly for
    // some cases. Feel free to extend the functions that are separate when
    // necessary. Ref https://stackoverflow.com/a/66077767/3064736
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        fold_transform(self, transform)
    }
    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        fold_func_def(self, function)
    }
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_call)
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
pub fn fold_item<T: ?Sized + AstFold>(fold: &mut T, item: Item) -> Result<Item> {
    Ok(match item {
        Item::Ident(ident) => Item::Ident(fold.fold_ident(ident)?),
        Item::Expr(items) => Item::Expr(fold.fold_nodes(items)?),
        Item::List(items) => Item::List(
            items
                .into_iter()
                .map(|x| fold.fold_node(x.into_inner()).map(ListItem))
                .try_collect()?,
        ),
        Item::Query(Query { nodes: items }) => Item::Query(Query {
            nodes: fold.fold_nodes(items)?,
        }),
        Item::InlinePipeline(InlinePipeline { value, functions }) => {
            Item::InlinePipeline(InlinePipeline {
                value: Box::from(fold.fold_node(*value)?),
                functions: fold.fold_nodes(functions)?,
            })
        }
        Item::Pipeline(transformations) => Item::Pipeline(fold.fold_pipeline(transformations)?),
        Item::NamedExpr(named_expr) => Item::NamedExpr(fold.fold_named_expr(named_expr)?),
        Item::Transform(transformation) => Item::Transform(fold.fold_transform(transformation)?),
        Item::SString(items) => Item::SString(
            items
                .into_iter()
                .map(|x| fold.fold_sstring_item(x))
                .try_collect()?,
        ),
        Item::FuncDef(func) => Item::FuncDef(fold.fold_func_def(func)?),
        Item::FuncCall(func_call) => Item::FuncCall(fold.fold_func_call(func_call)?),
        Item::Table(table) => Item::Table(Table {
            name: table.name,
            pipeline: fold.fold_pipeline(table.pipeline)?,
        }),
        // None of these capture variables, so we don't need to replace
        // them.
        Item::String(_) | Item::Raw(_) => item,
    })
}

pub fn fold_sstring_item<T: ?Sized + AstFold>(
    fold: &mut T,
    sstring_item: SStringItem,
) -> Result<SStringItem> {
    Ok(match sstring_item {
        SStringItem::String(string) => SStringItem::String(string),
        SStringItem::Expr(expr) => SStringItem::Expr(fold.fold_node(expr)?),
    })
}

pub fn fold_transform<T: ?Sized + AstFold>(
    fold: &mut T,
    transformation: Transform,
) -> Result<Transform> {
    match transformation {
        Transform::Derive(assigns) => Ok(Transform::Derive(fold.fold_nodes(assigns)?)),
        Transform::From(table) => Ok(Transform::From(fold.fold_table_ref(table)?)),
        Transform::Filter(Filter(items)) => Ok(Transform::Filter(Filter(fold.fold_nodes(items)?))),
        Transform::Sort(items) => Ok(Transform::Sort(fold.fold_nodes(items)?)),
        Transform::Join { side, with, filter } => Ok(Transform::Join {
            side,
            with: fold.fold_table_ref(with)?,
            filter: fold_join_filter(fold, filter)?,
        }),
        Transform::Select(items) => Ok(Transform::Select(fold.fold_nodes(items)?)),
        Transform::Aggregate { by, select } => Ok(Transform::Aggregate {
            by: fold.fold_nodes(by)?,
            select: fold.fold_nodes(select)?,
        }),
        // TODO: generalize? Or this never changes?
        Transform::Take(_) => Ok(transformation),
    }
}

pub fn fold_join_filter<T: ?Sized + AstFold>(fold: &mut T, f: JoinFilter) -> Result<JoinFilter> {
    Ok(match f {
        JoinFilter::On(nodes) => JoinFilter::On(fold.fold_nodes(nodes)?),
        JoinFilter::Using(nodes) => JoinFilter::Using(fold.fold_nodes(nodes)?),
    })
}

pub fn fold_func_call<T: ?Sized + AstFold>(fold: &mut T, func_call: FuncCall) -> Result<FuncCall> {
    Ok(FuncCall {
        // TODO: generalize? Or this never changes?
        name: func_call.name.to_owned(),
        args: func_call
            .args
            .into_iter()
            .map(|item| fold.fold_node(item))
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

pub fn fold_func_def<T: ?Sized + AstFold>(fold: &mut T, func_def: FuncDef) -> Result<FuncDef> {
    Ok(FuncDef {
        name: fold.fold_ident(func_def.name)?,
        positional_params: fold.fold_nodes(func_def.positional_params)?,
        named_params: fold.fold_nodes(func_def.named_params)?,
        body: Box::new(fold.fold_node(*func_def.body)?),
    })
}

pub fn fold_named_expr<T: ?Sized + AstFold>(
    fold: &mut T,
    named_expr: NamedExpr,
) -> Result<NamedExpr> {
    Ok(NamedExpr {
        name: fold.fold_ident(named_expr.name)?,
        expr: Box::new(fold.fold_node(*named_expr.expr)?),
    })
}
