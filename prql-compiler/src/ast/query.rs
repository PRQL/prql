/// Types for outer-scope AST nodes (query, table, func def, transform)
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::error::Span;

use super::{Dialect, Expr, Frame, Range, Ty};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Query {
    pub def: QueryDef,

    pub tables: Vec<Table>,
    pub main_pipeline: Vec<Transform>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub dialect: Dialect,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub id: Option<usize>,

    pub pipeline: Vec<Transform>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ResolvedQuery {
    pub transforms: Vec<Transform>,
}

/// Transform is a stage of a pipeline. It is created from a FuncCall during parsing.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub kind: TransformKind,

    /// True when transform contains window functions
    pub is_complex: bool,

    /// Result type
    pub ty: Frame,

    pub span: Option<Span>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum TransformKind {
    From(TableRef),
    Select(Vec<Expr>),
    Filter(Box<Expr>),
    Derive(Vec<Expr>),
    Aggregate {
        assigns: Vec<Expr>,
        by: Vec<Expr>,
    },
    Sort(Vec<ColumnSort<Expr>>),
    Take {
        range: Range,
        by: Vec<Expr>,
        sort: Vec<ColumnSort<Expr>>,
    },
    Join {
        side: JoinSide,
        with: TableRef,
        filter: JoinFilter,
    },
    Group {
        by: Vec<Expr>,
        pipeline: Vec<Transform>,
    },
    Window {
        kind: WindowKind,
        range: Range,
        pipeline: Vec<Transform>,
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
    On(Vec<Expr>),
    Using(Vec<Expr>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnSort<T = Expr> {
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
            span: None,
        }
    }
}
