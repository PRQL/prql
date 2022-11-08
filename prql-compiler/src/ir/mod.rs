mod expr;
mod id_gen;
mod ir_fold;
mod table_counter;

use enum_as_inner::EnumAsInner;
pub use expr::{Expr, ExprKind, UnOp};
pub use id_gen::IdGenerator;
pub use ir_fold::*;
pub use table_counter::TableCounter;

use serde::{Deserialize, Serialize};

use crate::ast::{ColumnSort, QueryDef, Range};
use crate::ast::{JoinSide, TableExternRef, WindowKind};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    pub def: QueryDef,

    pub tables: Vec<TableDef>,
    pub expr: TableExpr,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableDef {
    pub id: TId,

    /// Given name of this table (name of the CTE)
    pub name: Option<String>,

    pub expr: TableExpr,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TableRef {
    // referenced table
    pub source: TId,

    // new column definitions are required because there may be multiple instances
    // of this table in the same query
    pub columns: Vec<ColumnDef>,

    /// Given name of this table (table alias)
    pub name: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum TableExpr {
    ExternRef(TableExternRef, Vec<ColumnDef>),
    Pipeline(Vec<Transform>),
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum Transform {
    From(TableRef),
    Compute(ColumnDef),
    Select(Vec<CId>),
    Filter(Expr),
    Aggregate(Vec<CId>),
    Sort(Vec<ColumnSort<CId>>),
    Take(Range<Expr>),
    Join {
        side: JoinSide,
        with: TableRef,
        filter: Expr,
    },
    Unique,
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Window {
    kind: WindowKind,
    range: Range<CId>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub id: CId,
    pub kind: ColumnDefKind,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum ColumnDefKind {
    Wildcard,
    ExternRef(String),
    Expr { name: Option<String>, expr: Expr },
}

impl ColumnDef {
    pub fn get_name(&self) -> Option<&String> {
        match &self.kind {
            ColumnDefKind::Expr { name, .. } => name.as_ref(),
            _ => None,
        }
    }
}

/// Column id
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CId(usize);

impl From<usize> for CId {
    fn from(id: usize) -> Self {
        CId(id)
    }
}

impl std::fmt::Debug for CId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "column-{}", self.0)
    }
}

/// Table id
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TId(usize);

impl From<usize> for TId {
    fn from(id: usize) -> Self {
        TId(id)
    }
}

impl std::fmt::Debug for TId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table-{}", self.0)
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            kind: WindowKind::Rows,
            range: Range::unbounded(),
        }
    }
}
