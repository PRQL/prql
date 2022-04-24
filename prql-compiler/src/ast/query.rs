/// Types for outer-scope AST nodes (query, table, func def, transform)
use serde::{Deserialize, Serialize};
use strum::EnumString;

use super::{Dialect, Ident, Node};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Query {
    pub version: Option<String>,
    #[serde(default)]
    pub dialect: Dialect,
    pub nodes: Vec<Node>,
}

/// Function definition.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: Ident,
    #[serde(default)]
    pub kind: Option<FuncKind>,
    pub positional_params: Vec<Node>, // ident
    pub named_params: Vec<Node>,      // named expr
    pub body: Box<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumString)]
pub enum FuncKind {
    #[strum(serialize = "transform")]
    Transform,
    #[strum(serialize = "aggregation")]
    Aggregation,
    #[strum(serialize = "window")]
    Window,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub pipeline: Box<Node>,
}

/// Transform is a stage of a pipeline. It is created from a FuncCall during parsing.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum Transform {
    From(TableRef),
    Select(Select),
    Filter(Vec<Node>),
    Derive(Select),
    Aggregate(Select),
    Sort(Vec<ColumnSort<Node>>),
    Take(i64),
    Join {
        side: JoinSide,
        with: TableRef,
        filter: JoinFilter,
    },
    Group {
        by: Vec<Node>,
        pipeline: Box<Node>,
    },
}
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Select {
    pub assigns: Vec<Node>,
    pub group: Vec<Node>,
    pub window: Option<Vec<Node>>,
    pub sort: Option<Vec<Node>>,
}

impl Select {
    pub fn new(assigns: Vec<Node>) -> Self {
        Select {
            assigns,
            group: Vec::new(),
            window: None,
            sort: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum JoinFilter {
    On(Vec<Node>),
    Using(Vec<Node>),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnSort<T = Node> {
    pub direction: SortDirection,
    pub column: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Asc
    }
}
