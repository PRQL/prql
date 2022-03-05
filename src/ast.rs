use super::utils::*;
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
    // TODO: Add dialect & prql version onto Query.
    Query(Items),
    Pipeline(Pipeline),
    // Similar to holding an Expr, but we strongly type it so the parsing can be more strict.
    List(Vec<ListItem>),
    // Holds Items / Terms, not including separators like `+`. Unnesting this
    // (i.e. Items(Item) -> Item) does not change its semantics. (More detail in
    // `prql.pest`)
    // (possibly rename to Terms)
    Items(Items),
    // Holds any Items. Unnesting _can_ change semantics (though it's less
    // important than when this was used as a ListItem).
    // (possibly rename to Items)
    Expr(Items),
    Idents(Idents),
    Function(Function),
    Table(Table),
    SString(Vec<SStringItem>),
    // Anything not yet implemented.
    Todo(String),
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
    From(Items),
    Select(Items),
    Filter(Filter),
    Derive(Vec<Assign>),
    Aggregate {
        by: Items,
        // This is currently one list. TODO: change to a Vec of Items? One Items
        // would get unnested.
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
    /// Either provide a Vec with the contents of Item / Expr, or puts a scalar
    /// into a Vec. This is useful when we either have a scalar or a list, and
    /// want to only have to handle a single type.
    pub fn into_inner_items(self) -> Vec<Item> {
        match self {
            Item::Items(items) | Item::Expr(items) => items,
            _ => vec![self],
        }
    }
    pub fn as_inner_items(&self) -> Result<&Vec<Item>> {
        if let Item::Items(items) | Item::Expr(items) = self {
            Ok(items)
        } else {
            Err(anyhow!("Expected container type; got {:?}", self))
        }
    }
    pub fn into_inner_list_items(self) -> Result<Vec<Vec<Item>>> {
        match self {
            Item::List(items) => Ok(items.into_iter().map(|item| item.into_inner()).collect()),
            _ => Err(anyhow!("Expected a list, got {:?}", self)),
        }
    }
    /// For lists that only have one item in each ListItem this returns a Vec of
    /// those items. (e.g. `[1, a b]` but not `[1 + 2]`, because `+` in an
    /// operator and so will create an `Items` for each of `1` & `2`)
    pub fn into_inner_list_single_items(self) -> Result<Vec<Item>> {
        match self {
            Item::List(items) => items
                .into_iter()
                .map(|list_item| list_item.into_inner().into_only())
                .try_collect(),
            _ => Err(anyhow!("Expected a list, got {:?}", self)),
        }
    }

    /// Wrap in Items unless it's already an Items.
    pub fn coerce_to_items(self) -> Item {
        match self {
            Item::Items(_) => self,
            _ => Item::Items(vec![self]),
        }
    }
    /// Either provide a List with the contents of `self`, or `self` if the item
    /// is already a list. This is useful when we either have a scalar or a
    /// list, and want to only have to handle a single type.
    pub fn coerce_to_list(self) -> Item {
        match self {
            Item::List(_) => self,
            _ => Item::List(vec![ListItem(vec![self])]),
        }
    }
    /// Make a list from a vec of Items
    pub fn into_list_of_items(items: Items) -> Item {
        Item::List(items.into_iter().map(|item| ListItem(vec![item])).collect())
    }

    /// The scalar version / opposite of `as_inner_items`. It keeps unwrapping
    /// Item / Expr types until it finds one with a non-single element.
    // TODO: I can't seem to get a move version of this that works with the
    // `.unwrap_or` at the end — is there a way?
    pub fn as_scalar(&self) -> &Item {
        match self {
            Item::Items(items) | Item::Expr(items) => {
                items.only().map(|item| item.as_scalar()).unwrap_or(self)
            }
            _ => self,
        }
    }

    /// Transitively unnest the whole tree, traversing even parents with more
    /// than one child. This is more unnesting that `as_scalar' does. Only
    /// removes `Items` (not `Expr` or `List`), though it does walk all the
    /// containers.
    pub fn into_unnested(self) -> Item {
        match self {
            Item::Items(items) => {
                Item::Items(items.into_iter().map(|item| item.into_unnested()).collect())
                    .as_scalar()
                    .clone()
            }
            // Unpack, operate on, and then repack the items in a List.
            Item::List(_) => Item::List(
                self.into_inner_list_items()
                    .unwrap()
                    .into_iter()
                    .map(|list_item| {
                        ListItem(list_item.into_iter().map(|x| x.into_unnested()).collect())
                    })
                    .collect(),
            ),
            Item::Expr(items) => {
                Item::Expr(items.into_iter().map(|item| item.into_unnested()).collect())
            }
            _ => self,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_as_scalar() {
        let atom = Item::Ident("a".to_string());

        // Gets the single item through one level of nesting.
        let item = Item::Items(vec![atom.clone()]);
        assert_eq!(item.as_scalar(), &atom);

        // No change when it's the same.
        let item = atom.clone();
        assert_eq!(item.as_scalar(), &item);

        // No change when there are two items in the `items`.
        let item = Item::Items(vec![atom.clone(), atom.clone()]);
        assert_eq!(item.as_scalar(), &item);

        // Gets the single item through two levels of nesting.
        let item = Item::Items(vec![Item::Items(vec![atom.clone()])]);
        assert_eq!(item.as_scalar(), &atom);
    }
}
