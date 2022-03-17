use super::utils::*;
use crate::ast_fold::AstFold;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

// Idents are generally columns
pub type Ident = String;
pub type Items = Vec<Item>;
pub type Idents = Vec<Ident>;
pub type Pipeline = Vec<Transformation>;

use enum_as_inner::EnumAsInner;

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Transformation(Transformation),
    Ident(Ident),
    String(String),
    Raw(String),
    Assign(Assign),
    NamedArg(NamedArg),
    Query(Query),
    Pipeline(Pipeline),
    // Currently this is separate from `Pipeline`, but we could unify them at
    // some point. We'll need to relax the constraints on `Pipeline` to allow it
    // to start with a simple expression.
    InlinePipeline(Items),
    // Similar to holding an Expr, but we strongly type it so the parsing can be more strict.
    List(Vec<ListItem>),
    // Holds "Terms", not including separators like `+`. Unnesting this (i.e.
    // Terms([Item]) -> Item) does not change its semantics. (More detail in
    // `prql.pest`)
    Terms(Items),
    // Holds any Items. Unnesting _can_ change semantics (though it's less
    // important than when this was used as a ListItem).
    Items(Items),
    Idents(Idents),
    Function(Function),
    Table(Table),
    SString(Vec<SStringItem>),
    // Anything not yet implemented.
    Todo(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    // TODO: Add dialect & prql version onto Query.
    pub items: Items,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Items);

impl ListItem {
    pub fn into_inner(self) -> Items {
        self.0
    }
}

/// Transformation is currently used for a) each transformation in a pipeline
/// and sometimes b) a normal function call. But we want to resolve whether (b)
/// should apply or not.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
// We probably want to implement some of these as Structs rather than just
// `Items`
pub enum Transformation {
    From(Ident),
    Select(Items),
    Filter(Filter),
    Derive(Vec<Assign>),
    Aggregate {
        by: Vec<Item>,
        calcs: Vec<Item>,
        assigns: Vec<Assign>,
    },
    Sort(Items),
    Take(i64),
    Join(Items),
    Func(FuncCall),
}

impl Transformation {
    /// Returns the name of the transformation.
    pub fn name(&self) -> &str {
        match self {
            Transformation::From(_) => "from",
            Transformation::Select(_) => "select",
            Transformation::Filter(_) => "filter",
            Transformation::Derive(_) => "derive",
            Transformation::Aggregate { .. } => "aggregate",
            Transformation::Sort(_) => "sort",
            Transformation::Take(_) => "take",
            Transformation::Join(_) => "join",
            // Currently this is unused, since we don't encode function calls as
            // anything more than Idents at the moment. We may want to change
            // that in the future.
            Transformation::Func(FuncCall { name, .. }) => name,
        }
    }
}

/// Function definition.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub body: Items,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: String,
    pub args: Items,
    pub named_args: Vec<NamedArg>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: Ident,
    pub pipeline: Pipeline,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedArg {
    pub name: Ident,
    pub arg: Box<Item>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Assign {
    pub lvalue: Ident,
    pub rvalue: Box<Item>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SStringItem {
    String(String),
    Expr(Item),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Filter(pub Items);

// We've done a lot of iteration on these containers, and it's still very messy.
// Some of the tradeoff is having an Enum which is flexible, but not falling
// back to dynamic types, which makes understanding what the parser is doing
// more difficult.
impl Item {
    /// Either provide a Vec with the contents of Items / Terms / Query, or puts a scalar
    /// into a Vec. This is useful when we either have a scalar or a list, and
    /// want to only have to handle a single type.
    pub fn into_inner_items(self) -> Vec<Item> {
        match self {
            Item::Terms(items) | Item::Items(items) | Item::Query(Query { items }) => items,
            _ => vec![self],
        }
    }
    pub fn as_inner_items(&self) -> Result<&Vec<Item>> {
        if let Item::Terms(items) | Item::Items(items) = self {
            Ok(items)
        } else if let Item::Query(Query { items }) = self {
            Ok(items)
        } else {
            Err(anyhow!("Expected container type; got {self:?}"))
        }
    }
    pub fn into_inner_terms(self) -> Vec<Item> {
        match self {
            Item::Terms(terms) => terms,
            _ => vec![self],
        }
    }
    pub fn into_inner_list_items(self) -> Result<Vec<Vec<Item>>> {
        match self {
            Item::List(items) => Ok(items.into_iter().map(|item| item.into_inner()).collect()),
            _ => Err(anyhow!("Expected a list, got {self:?}")),
        }
    }
    /// For lists that only have one item in each ListItem this returns a Vec of
    /// those terms. (e.g. `[1, a b]` but not `[1 + 2]`, because `+` in an
    /// operator and so will create an `Items` for each of `1` & `2`)
    pub fn into_inner_list_single_items(self) -> Result<Vec<Item>> {
        match self {
            Item::List(items) => items
                .into_iter()
                .map(|list_item| list_item.into_inner().into_only())
                .try_collect(),
            _ => Err(anyhow!("Expected a list of single items, got {self:?}")),
        }
    }

    /// Either provide a List with the contents of `self`, or `self` if the item
    /// is already a list. This is useful when we either have a scalar or a
    /// list, and want to only have to handle a single type.
    fn coerce_to_list(self) -> Item {
        match self {
            Item::List(_) => self,
            _ => Item::List(vec![ListItem(vec![self])]),
        }
    }
    /// Make a list from a vec of Items
    pub fn into_list_of_items(items: Items) -> Item {
        Item::List(items.into_iter().map(|item| ListItem(vec![item])).collect())
    }
    /// Often we don't care whether a List or single item is passed; e.g.
    /// `select x` vs `select [x, y]`. This equalizes them both to a vec of
    /// Items, including unnesting any ListItems.
    pub fn into_items_from_maybe_list(self) -> Items {
        self.coerce_to_list()
            .into_inner_list_items()
            .unwrap()
            .into_iter()
            .map(Item::Terms)
            .map(|x| x.into_unnested())
            .collect()
    }
}

pub trait IntoUnnested {
    fn into_unnested(self) -> Self;
}
impl IntoUnnested for Item {
    /// Transitively unnest the whole tree, traversing even parents with more
    /// than one child. This is more unnesting that `as_scalar' does. Only
    /// removes `Terms` (not `Items` or `List`), though it does walk all the
    /// containers.
    fn into_unnested(self) -> Self {
        Unnest.fold_item(self).unwrap()
    }
}

use super::ast_fold::fold_item;
struct Unnest;
impl AstFold for Unnest {
    // TODO: We could make this Infallible
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        match item {
            Item::Terms(terms) => {
                // Possibly this can be more elegant. One issue with combining
                // these into a single statement is we can't use `self` twice,
                // which I think isn't avoidable.

                // Get the inner items, passing each of those to `fold_item`.
                let inner_terms = terms
                    .into_iter()
                    .map(|term| self.fold_item(term).unwrap())
                    .collect::<Vec<Item>>();

                // If there's only one item, pass it to `fold_item`, otherwise
                // pass all the items.
                fold_item(
                    self,
                    inner_terms
                        // We need this clone because of the `unwrap_or` below. An
                        // alternative approach would be to test whether it's the only
                        // item without moving it.
                        .clone()
                        .into_only()
                        .unwrap_or(Item::Terms(inner_terms)),
                )
            }
            _ => fold_item(self, item),
        }
    }
}

use anyhow::Error;
impl From<Item> for Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    fn from(item: Item) -> Self {
        anyhow!("Failed to convert {item:?}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_into_unnested() {
        let atom = Item::Ident("a".to_string());
        let single_term = Item::Terms(vec![atom.clone()]);
        let single_item = Item::Items(vec![atom.clone()]);

        // Gets the single item through one level of nesting.
        let item = single_term.clone();
        assert_eq!(item.into_unnested(), atom);

        // Doesn't break through an Items.
        let item = single_item.clone();
        assert_eq!(&item.clone().into_unnested(), &item);

        // `Terms -> Items -> Terms` goes to `Items -> Terms`
        let item = Item::Terms(vec![Item::Items(vec![single_term.clone()])]);
        assert_eq!(item.into_unnested(), single_item);

        // No change on a simple ident.
        let item = atom.clone();
        assert_eq!(item.clone().into_unnested(), item);

        // No change when there are two items in the `terms`.
        let item = Item::Terms(vec![atom.clone(), atom.clone()]);
        assert_eq!(item.clone().into_unnested(), item);

        // Gets the single item through two levels of nesting.
        let item = Item::Terms(vec![single_term.clone()]);
        assert_eq!(item.into_unnested(), atom);

        // Gets a single item through a parent which isn't nested
        let item = Item::Terms(vec![single_term.clone(), single_term.clone()]);
        assert_eq!(item.into_unnested(), Item::Terms(vec![atom.clone(), atom]));

        dbg!(single_term);
    }
}
