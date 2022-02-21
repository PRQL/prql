use anyhow::{anyhow, Result};

use serde::{Deserialize, Serialize};

// Idents are generally columns
pub type Ident = String;
pub type Items = Vec<Item>;
pub type Idents = Vec<Ident>;
pub type Pipeline = Vec<Transformation>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Transformation(Transformation),
    Ident(Ident),
    String(String),
    Raw(String),
    Assign(Assign),
    NamedArg(NamedArg),
    Query(Items),
    Pipeline(Pipeline),
    // Holds Item-s directly if a list entry is a single item, otherwise holds
    // Item::Items. This is less verbose than always having Item::Items.
    List(Items),
    // In some cases, as as lists, we need a container for multiple items to
    // discriminate them from, e.g. a series of Idents. `[a, b]` vs `[a b]`.
    Items(Items),
    Idents(Idents),
    Function(Function),
    Table(Table),
    // Anything not yet implemented.
    TODO(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
// Need to implement some of these as Structs rather than just `Items`
pub enum Transformation {
    From(Items),
    Select(Items),
    Filter(Filter),
    Derive(Vec<Assign>),
    Aggregate {
        by: Items,
        calcs: Items,
    },
    // TODO: add ordering
    Sort(Items),
    Take(i64),
    Join(Items),
    Custom {
        name: String,
        args: Items,
        named_args: Vec<NamedArg>,
    },
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
            Transformation::Custom { name, .. } => name,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub body: Items,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: Ident,
    pub pipeline: Pipeline,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedArg {
    pub lvalue: Ident,
    // TODO: I think this should just be a single Item, which requires boxing it.
    pub rvalue: Items,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Assign {
    pub lvalue: Ident,
    // TODO: I think this should just be a single Item, which requires boxing it.
    pub rvalue: Items,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Filter(pub Items);

impl Item {
    /// Either provide a Vec with the contents of List / Item, or puts a scalar
    /// into a Vec. This is useful when we either have a scalar or a list, and
    /// want to only have to handle a single type.
    #[must_use]
    pub fn to_items(&self) -> Vec<Item> {
        match self {
            Item::List(items) | Item::Items(items) => items.clone(),
            _ => vec![self.clone()],
        }
    }

    // We could expand these with (but it will add lots of methods...)
    // https://crates.io/crates/enum-as-inner?
    pub fn as_ident(&self) -> Result<&Ident> {
        if let Item::Ident(ident) = self {
            Ok(ident)
        } else {
            Err(anyhow!("Expected Item::Ident, got {:?}", self))
        }
    }
    pub fn as_named_arg(&self) -> Result<&NamedArg> {
        if let Item::NamedArg(named_arg) = self {
            Ok(named_arg)
        } else {
            Err(anyhow!("Expected Item::NamedArg, got {:?}", self))
        }
    }
    pub fn as_assign(&self) -> Result<&Assign> {
        if let Item::Assign(assign) = self {
            Ok(assign)
        } else {
            Err(anyhow!("Expected Item::Assign, got {:?}", self))
        }
    }
    pub fn as_raw(&self) -> Result<&String> {
        if let Item::Raw(raw) = self {
            Ok(raw)
        } else {
            Err(anyhow!("Expected Item::Raw, got {:?}", self))
        }
    }
}
