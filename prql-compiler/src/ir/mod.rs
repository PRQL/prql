mod expr;
mod id_gen;
mod ir_fold;

pub use expr::{Expr, ExprKind, UnOp};
pub use id_gen::IdGenerator;
pub use ir_fold::*;

use serde::{Deserialize, Serialize};

use crate::ast::{ColumnSort, QueryDef, Range};
use crate::ast::{JoinSide, TableRef, WindowKind};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Query {
    pub def: QueryDef,

    pub tables: Vec<Table>,
    pub expr: TableExpr,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub id: TId,

    /// Given name of this table.
    pub name: Option<String>,

    pub expr: TableExpr,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum TableExpr {
    Ref(TableRef, Vec<ColumnDef>),
    Pipeline(Vec<Transform>),
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum Transform {
    From(TId),
    Compute(ColumnDef),
    Select(Vec<CId>),
    Filter(Expr),
    Aggregate(Vec<CId>),
    Sort(Vec<ColumnSort<CId>>),
    Take(Range<Expr>),
    Join {
        side: JoinSide,
        with: TId,
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
    pub name: Option<String>,
    pub expr: Expr,
}

/// Column id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CId(usize);

impl CId {
    pub fn new(id: usize) -> Self {
        CId(id)
    }
}

/// Table id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TId(usize);

impl TId {
    pub fn new(id: usize) -> Self {
        TId(id)
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
