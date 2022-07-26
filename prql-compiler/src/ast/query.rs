/// Types for outer-scope AST nodes (query, table, func def, transform)
use serde::{Deserialize, Serialize};

use super::{Dialect, Frame, Ident, Node, Range, Ty};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Query {
    pub version: Option<i64>,
    #[serde(default)]
    pub dialect: Dialect,
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ResolvedQuery {
    pub transforms: Vec<Transform>,
}

/// Function definition.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: Ident,
    pub positional_params: Vec<FuncParam>, // ident
    pub named_params: Vec<FuncParam>,      // named expr
    pub body: Box<Node>,
    pub return_ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    pub default_value: Option<Node>,

    #[serde(skip)]
    pub declared_at: Option<usize>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub pipeline: Box<Node>,
    pub id: Option<usize>,
}

/// Transform is a stage of a pipeline. It is created from a FuncCall during parsing.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub kind: TransformKind,

    /// True when transform contains window functions
    pub is_complex: bool,

    /// Result type
    pub ty: Frame,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum TransformKind {
    From(TableRef),
    Select(Vec<Node>),
    Filter(Box<Node>),
    Derive(Vec<Node>),
    Aggregate {
        assigns: Vec<Node>,
        by: Vec<Node>,
    },
    Sort(Vec<ColumnSort<Node>>),
    Take {
        range: Range,
        by: Vec<Node>,
        sort: Vec<ColumnSort<Node>>,
    },
    Join {
        side: JoinSide,
        with: TableRef,
        filter: JoinFilter,
    },
    Group {
        by: Vec<Node>,
        pipeline: ResolvedQuery,
    },
    Window {
        kind: WindowKind,
        range: Range,
        pipeline: ResolvedQuery,
    },
    Unique, // internal only, can be expressed with group & take
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum WindowKind {
    Rows,
    Range,
}
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
    pub declared_at: Option<usize>,
    pub ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum JoinFilter {
    On(Vec<Node>),
    Using(Vec<Node>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnSort<T = Node> {
    pub direction: SortDirection,
    pub column: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Asc
    }
}

impl From<TransformKind> for Transform {
    fn from(kind: TransformKind) -> Self {
        Transform {
            kind,
            is_complex: false,
            ty: Frame::default(),
        }
    }
}
