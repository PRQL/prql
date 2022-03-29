use anyhow::{anyhow, bail, Result};
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

use crate::error::{Error, Reason, Span};
use crate::utils::*;

/// A name. Generally columns, tables, functions, variables.
pub type Ident = String;
pub type Pipeline = Vec<Transformation>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Node {
    #[serde(flatten)]
    pub item: Item,
    #[serde(skip)]
    pub span: Span,
    #[serde(skip)]
    pub declared_at: Option<usize>,
}

#[derive(Debug, EnumAsInner, Display, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Transformation(Transformation),
    Ident(Ident),
    String(String),
    Raw(String),
    NamedExpr(NamedExpr),
    Query(Query),
    Pipeline(Pipeline),
    // Currently this is separate from `Pipeline`, but we could unify them at
    // some point. We'll need to relax the constraints on `Pipeline` to allow it
    // to start with a simple expression.
    InlinePipeline(InlinePipeline),
    List(Vec<ListItem>),
    Expr(Vec<Node>),
    FuncDef(FuncDef),
    FuncCall(FuncCall),
    Table(Table),
    SString(Vec<SStringItem>),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    // TODO: Add dialect & prql version onto Query.
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Node);

impl ListItem {
    pub fn into_inner(self) -> Node {
        self.0
    }
}

/// Transformation is used for each stage in a pipeline
/// and sometimes
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
// We probably want to implement some of these as Structs rather than just
// `vec<Item>`
pub enum Transformation {
    From(TableRef),
    Select(Vec<Node>),
    Filter(Filter),
    Derive(Vec<Node>),
    Aggregate {
        by: Vec<Node>,
        select: Vec<Node>,
    },
    Sort(Vec<Node>),
    Take(i64),
    Join {
        side: JoinSide,
        with: TableRef,
        on: Vec<Node>,
    },
}

impl Transformation {
    /// Returns the name of the transformation.
    pub fn name(&self) -> &'static str {
        match self {
            Transformation::From(_) => "from",
            Transformation::Select(_) => "select",
            Transformation::Filter(_) => "filter",
            Transformation::Derive(_) => "derive",
            Transformation::Aggregate { .. } => "aggregate",
            Transformation::Sort(_) => "sort",
            Transformation::Take(_) => "take",
            Transformation::Join { .. } => "join",
        }
    }
}

/// Function definition.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: Ident,
    pub positional_params: Vec<Ident>,
    pub named_params: Vec<NamedExpr>,
    pub body: Box<Node>,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Ident,
    pub args: Vec<Node>,
    pub named_args: Vec<NamedExpr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct InlinePipeline {
    pub value: Box<Node>,
    pub functions: Vec<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub pipeline: Pipeline,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedExpr {
    pub name: Ident,
    pub expr: Box<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SStringItem {
    String(String),
    Expr(Item),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Filter(pub Vec<Node>);

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
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
        if let Item::NamedExpr(expr) = self.item {
            (Some(expr.name), *expr.expr)
        } else {
            (None, self)
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
            span: Span::default(),
            declared_at: None,
        }
    }
}

impl From<Item> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    fn from(item: Item) -> Self {
        anyhow!("Failed to convert {item:?}")
    }
}
