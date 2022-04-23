/// Abstract syntax tree for PRQL language
///
/// The central struct here is [Node], that can be of different kinds, described with [item::Item].
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub use self::dialect::*;
pub use self::item::*;
pub use self::query::*;
use crate::error::{Error, Reason, Span};
use crate::semantic::Frame;
use crate::utils::*;

pub mod ast_fold;
pub mod dialect;
pub mod item;
pub mod query;

pub fn display(query: Query) -> String {
    format!("{}", Item::Query(query))
}

/// A name. Generally columns, tables, functions, variables.
pub type Ident = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    #[serde(flatten)]
    pub item: Item,
    #[serde(skip)]
    pub span: Option<Span>,
    #[serde(skip)]
    pub declared_at: Option<usize>,
    #[serde(skip)]
    pub frame: Option<Frame>,
}

impl Node {
    /// For lists that only have one item in each ListItem this returns a Vec of
    /// those terms. (e.g. `[1, a b]` but not `[1 + 2]`, because `+` in an
    /// operator and so will create an `Items` for each of `1` & `2`)
    pub fn into_inner_list_nodes(self) -> Result<Vec<Node>> {
        Ok(match self.item {
            Item::List(items) => items.into_iter().map(|x| x.into_inner()).collect(),
            _ => bail!("Expected a list of single items, got {self:?}"),
        })
    }

    /// Make a List from a vec of Items
    pub fn into_list_of_nodes(node: Vec<Node>) -> Node {
        Item::List(node.into_iter().map(ListItem).collect()).into()
    }

    /// Return an error if this is named expression.
    pub fn discard_name(self) -> Result<Node, Error> {
        if let Item::NamedExpr(_) = self.item {
            Err(Error::new(Reason::Unexpected {
                found: "alias".to_string(),
            })
            .with_span(self.span))
        } else {
            Ok(self)
        }
    }

    pub fn into_name_and_expr(self) -> (Option<Ident>, Node) {
        // unwrap expr with only one child
        let expr = if let Item::Expr(mut expr) = self.item {
            expr.remove(0)
        } else {
            self
        };

        if let Item::NamedExpr(expr) = expr.item {
            (Some(expr.name), *expr.expr)
        } else {
            (None, expr)
        }
    }

    /// Often we don't care whether a List or single item is passed; e.g.
    /// `select x` vs `select [x, y]`. This equalizes them both to a vec of
    /// Item-ss.
    pub fn coerce_to_items(self) -> Vec<Node> {
        match self.item {
            Item::List(items) => items.into_iter().map(|x| x.into_inner()).collect(),
            _ => vec![self],
        }
    }

    pub fn unwrap<T, F>(self, f: F, expected: &str) -> Result<T, Error>
    where
        F: FnOnce(Item) -> Result<T, Item>,
    {
        f(self.item).map_err(|i| {
            Error::new(Reason::Expected {
                who: None,
                expected: expected.to_string(),
                found: i.to_string(),
            })
            .with_span(self.span)
        })
    }
}

/// Unnest Expr([x]) into x.
pub trait IntoExpr {
    fn into_expr(self) -> Item;
}
impl IntoExpr for Vec<Node> {
    fn into_expr(self) -> Item {
        if self.len() == 1 {
            self.into_only().unwrap().item
        } else {
            Item::Expr(self)
        }
    }
}

impl From<Item> for Node {
    fn from(item: Item) -> Self {
        Node {
            item,
            span: None,
            declared_at: None,
            frame: None,
        }
    }
}
