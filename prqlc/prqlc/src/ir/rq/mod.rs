//! Relational Query AST
//!
//! Strictly typed AST for describing relational queries.

mod expr;
mod fold;
mod ids;
mod transform;
mod utils;

use enum_as_inner::EnumAsInner;
pub use expr::{Expr, ExprKind, UnOp};
use expr::{InterpolateItem, Range, SwitchCase};
pub use fold::*;
pub use ids::*;
use serde::{Deserialize, Serialize};
pub use transform::*;
pub use utils::*;

use super::pl::TableExternRef;
use super::pl::{Literal, QueryDef};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct RelationalQuery {
    pub def: QueryDef,

    pub tables: Vec<TableDecl>,
    pub relation: Relation,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,

    /// Column definitions.
    /// This is the interface of the table that can be referenced from other tables.
    pub columns: Vec<RelationColumn>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum RelationKind {
    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map")
    )]
    ExternRef(TableExternRef),
    Pipeline(Vec<Transform>),
    Literal(RelationLiteral),
    SString(Vec<InterpolateItem>),
    BuiltInFunction {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct RelationLiteral {
    /// Column names
    pub columns: Vec<String>,
    /// Row-oriented data
    pub rows: Vec<Vec<Literal>>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, Serialize, Deserialize, EnumAsInner)]
pub enum RelationColumn {
    /// A single column that may have a name.
    /// Unnamed columns cannot be referenced.
    Single(Option<String>),

    /// Means "and other unmentioned columns". Does not mean "all columns".
    Wildcard,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableDecl {
    /// An id for this table, unique within all tables in this query.
    pub id: TId,

    /// Name hint for this declaration (name of the CTE)
    pub name: Option<String>,

    /// Table's contents.
    pub relation: Relation,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    /// Referenced table
    pub source: TId,

    /// New column definitions are required because there may be multiple instances
    /// of this table in the same query
    pub columns: Vec<(RelationColumn, CId)>,

    /// Name hint for relation within this pipeline (table alias)
    pub name: Option<String>,
}
