//! Relational Query
//!
//! Strictly typed AST for descibing relational queries.

mod expr;
mod fold;
mod ids;
mod transform;

pub use expr::{Expr, ExprKind, UnOp};
pub use fold::*;
pub use ids::*;
pub use transform::*;

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::pl::{ColumnSort, QueryDef, Range, WindowFrame};
use super::pl::{InterpolateItem, TableExternRef};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    pub def: QueryDef,

    pub tables: Vec<TableDecl>,
    pub relation: Relation,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,

    pub columns: Vec<RelationColumn>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum RelationKind {
    ExternRef(TableExternRef),
    Pipeline(Vec<Transform>),
    Literal(RelationLiteral),
    SString(Vec<InterpolateItem<Expr>>),
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, Serialize, Deserialize)]
pub enum RelationColumn {
    Wildcard,
    Single(Option<String>),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableDecl {
    pub id: TId,

    /// Given name of this table (name of the CTE)
    pub name: Option<String>,

    pub relation: Relation,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    // referenced table
    pub source: TId,

    // new column definitions are required because there may be multiple instances
    // of this table in the same query
    pub columns: Vec<(RelationColumn, CId)>,

    /// Given name of this table (table alias)
    pub name: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct RelationLiteral {
    /// Column names
    pub columns: Vec<String>,
    /// Row-oriented data
    // TODO: this should be generic, so it can contain any type (but at least
    // numbers)
    pub rows: Vec<Vec<String>>,
}
