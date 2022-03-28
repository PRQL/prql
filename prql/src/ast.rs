use anyhow::{anyhow, bail, Result};
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::utils::*;

// Idents are generally columns
pub type Ident = String;
pub type Pipeline = Vec<Transformation>;

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
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
    // Similar to holding an Expr, but we strongly type it so the parsing can be more strict.
    List(Vec<ListItem>),
    // Holds any Items. Unnesting _can_ change semantics.
    Expr(Vec<Item>),
    FuncDef(FuncDef),
    FuncCall(FuncCall),
    Table(Table),
    SString(Vec<SStringItem>),
    // Anything not yet implemented.
    Todo(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    // TODO: Add dialect & prql version onto Query.
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Item);

impl ListItem {
    pub fn into_inner(self) -> Item {
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
    From(TableRef),
    Select(Vec<Item>),
    Filter(Filter),
    Derive(Vec<Item>),
    Aggregate {
        by: Vec<Item>,
        select: Vec<Item>,
    },
    Sort(Vec<Item>),
    Take(i64),
    Join {
        side: JoinSide,
        with: Ident,
        on: Vec<Item>,
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
    pub body: Box<Item>,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: String,
    pub args: Vec<Item>,
    pub named_args: Vec<NamedExpr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct InlinePipeline {
    pub value: Box<Item>,
    pub functions: Vec<FuncCall>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: Ident,
    pub pipeline: Pipeline,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedExpr {
    pub name: Ident,
    pub expr: Box<Item>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SStringItem {
    String(String),
    Expr(Item),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Filter(pub Vec<Item>);

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    pub name: Ident,
    pub alias: Option<Ident>,
}

// We've done a lot of iteration on these containers, and it's still very messy.
// Some of the tradeoff is having an Enum which is flexible, but not falling
// back to dynamic types, which makes understanding what the parser is doing
// more difficult.
impl Item {
    /// For lists that only have one item in each ListItem this returns a Vec of
    /// those terms. (e.g. `[1, a b]` but not `[1 + 2]`, because `+` in an
    /// operator and so will create an `Items` for each of `1` & `2`)
    pub fn into_inner_list_items(self) -> Result<Vec<Item>> {
        Ok(match self {
            Item::List(items) => items.into_iter().map(|x| x.into_inner()).collect(),
            _ => bail!("Expected a list of single items, got {self:?}"),
        })
    }

    /// Make a List from a vec of Items
    pub fn into_list_of_items(items: Vec<Item>) -> Item {
        Item::List(items.into_iter().map(ListItem).collect())
    }

    /// Return an error if this is named expression.
    pub fn discard_name(self) -> Result<Item> {
        if let Item::NamedExpr(expr) = self {
            Err(anyhow!("Cannot use alias for: {expr:?}"))
        } else {
            Ok(self)
        }
    }

    pub fn into_name_and_expr(self) -> (Option<Ident>, Item) {
        if let Item::NamedExpr(expr) = self {
            (Some(expr.name), *expr.expr)
        } else {
            (None, self)
        }
    }

    /// Often we don't care whether a List or single item is passed; e.g.
    /// `select x` vs `select [x, y]`. This equalizes them both to a vec of
    /// Item-ss.
    pub fn coerce_to_items(self) -> Vec<Item> {
        match self {
            Item::List(items) => items.into_iter().map(|x| x.into_inner()).collect(),
            x => vec![x],
        }
    }
}

/// Unnest Expr([x]) into x.
pub trait IntoExpr {
    fn into_expr(self) -> Item;
}
impl IntoExpr for Vec<Item> {
    fn into_expr(self) -> Item {
        if self.len() == 1 {
            self.into_only().unwrap()
        } else {
            Item::Expr(self)
        }
    }
}

impl From<Item> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    fn from(item: Item) -> Self {
        anyhow!("Failed to convert {item:?}")
    }
}
