use std::collections::HashMap;

use anyhow::anyhow;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::{self, Display};

pub use super::*;

#[derive(Debug, EnumAsInner, Display, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Ident(Ident),
    String(String),
    Raw(String),
    NamedExpr(NamedExpr),
    Query(Query),
    Pipeline(Pipeline),
    Transform(Transform),
    List(Vec<ListItem>),
    Range(Range),
    Expr(Vec<Node>),
    FuncDef(FuncDef),
    FuncCall(FuncCall),
    Table(Table),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Interval(Interval),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Node);

impl ListItem {
    pub fn into_inner(self) -> Node {
        self.0
    }
}

/// Function call.
///
/// Note that `named_args` cannot be determined during parsing, but only during resolving.
/// Until then, they are stored in args as named expression.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Ident,
    pub args: Vec<Node>,
    pub named_args: HashMap<Ident, Box<Node>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub value: Option<Box<Node>>,
    pub functions: Vec<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedExpr {
    pub name: Ident,
    pub expr: Box<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum InterpolateItem {
    String(String),
    Expr(Node),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Range {
    pub start: Option<Box<Node>>,
    pub end: Option<Box<Node>>,
}

// I could imagine there being a wrapper of this to represent "2 days 3 hours".
// Or should that be written as `2days + 3hours`?
//
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct Interval(pub Vec<IntervalPart>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    pub n: i64,       // Do any DBs use floats or decimals for this?
    pub unit: String, // Could be an enum IntervalType,
}

impl Pipeline {
    pub fn into_transforms(self) -> Result<Vec<Transform>, Item> {
        self.functions
            .into_iter()
            .map(|f| f.item.into_transform())
            .try_collect()
    }

    pub fn as_transforms(&self) -> Option<Vec<&Transform>> {
        self.functions
            .iter()
            .map(|f| f.item.as_transform())
            .collect()
    }
}
impl From<Vec<Node>> for Pipeline {
    fn from(functions: Vec<Node>) -> Self {
        let value = None;
        Pipeline { functions, value }
    }
}

impl From<Item> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(item: Item) -> Self {
        #[cfg(debug_assertions)]
        {
            // panic!("Failed to convert {item:?}")
        }
        anyhow!("Failed to convert {item:?}")
    }
}
