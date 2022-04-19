/// Types for outer-scope AST nodes (query, table, func def, transform)

use serde::{Deserialize, Serialize};

use super::{Dialect, Node, Ident};


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
    pub positional_params: Vec<Node>, // ident
    pub named_params: Vec<Node>,      // named expr
    pub body: Box<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub pipeline: Vec<Transform>,
}

/// Transformation is used for each stage in a pipeline
/// and sometimes
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
// We probably want to implement some of these as Structs rather than just
// `vec<Item>`
pub enum Transform {
    From(TableRef),
    Select(Vec<Node>),
    Filter(Vec<Node>),
    Derive(Vec<Node>),
    Aggregate(Vec<Node>),
    Sort(Vec<ColumnSort<Node>>),
    Take(i64),
    Join {
        side: JoinSide,
        with: TableRef,
        filter: JoinFilter,
    },
    Group {
        by: Vec<Node>,
        pipeline: Vec<Transform>,
    },
}

impl Transform {
    /// Returns the name of the transformation.
    pub fn name(&self) -> &'static str {
        match self {
            Transform::From(_) => "from",
            Transform::Select(_) => "select",
            Transform::Filter(_) => "filter",
            Transform::Derive(_) => "derive",
            Transform::Aggregate { .. } => "aggregate",
            Transform::Sort(_) => "sort",
            Transform::Take(_) => "take",
            Transform::Join { .. } => "join",
            Transform::Group { .. } => "group",
        }
    }

    pub fn first_node(&self) -> Option<&Node> {
        match &self {
            Transform::From(_) => None,
            Transform::Select(nodes)
            | Transform::Filter(nodes)
            | Transform::Derive(nodes)
            | Transform::Aggregate(nodes) => nodes.first(),
            Transform::Sort(columns) => columns.first().map(|c| &c.column),
            Transform::Join { filter, .. } => filter.nodes().first(),
            Transform::Group { by, .. } => by.first(),
            Transform::Take(_) => None,
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

impl JoinFilter {
    fn nodes(&self) -> &Vec<Node> {
        match self {
            JoinFilter::On(nodes) => nodes,
            JoinFilter::Using(nodes) => nodes,
        }
    }
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
