/// Abstract syntax tree for PRQL language
///
/// The central struct here is [Node], that can be of different kinds, described with [item::Item].
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub use self::dialect::*;
pub use self::item::*;
pub use self::query::*;
use crate::error::{Error, Reason, Span};
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
}

impl Node {
    pub fn new_ident<S: ToString>(name: S, declared_at: usize) -> Node {
        let mut node: Node = Item::Ident(name.to_string()).into();
        node.declared_at = Some(declared_at);
        node
    }

    /// Return an error if this is named expression.
    pub fn discard_name(self) -> Result<Node, Error> {
        // TODO: replace this function with a prior type checking

        if let Item::Assign(_) = self.item {
            Err(Error::new(Reason::Unexpected {
                found: "alias".to_string(),
            })
            .with_span(self.span))
        } else {
            Ok(self)
        }
    }

    pub fn into_name_and_expr(self) -> (Option<Ident>, Node) {
        if let Item::Assign(expr) = self.item {
            (Some(expr.name), *expr.expr)
        } else {
            (None, self)
        }
    }

    /// Often we don't care whether a List or single item is passed; e.g.
    /// `select x` vs `select [x, y]`. This equalizes them both to a vec of
    /// [Node]-s.
    pub fn coerce_to_vec(self) -> Vec<Node> {
        match self.item {
            Item::List(items) => items,
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

/// Wrap into [Item::Expr] if there is more than one term.
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
        }
    }
}
