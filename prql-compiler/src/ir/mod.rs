mod expr;
mod ir_fold;
mod id_gen;

pub use ir_fold::*;
pub use expr::{Expr, ExprKind};
pub use id_gen::IdGenerator;

use serde::{Deserialize, Serialize};

use crate::{ast::{WindowKind, TableRef, JoinSide, JoinFilter}};
use crate::ast::{ColumnSort, QueryDef, Range};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Query {
    pub def: QueryDef,

    pub tables: Vec<Table>,
    pub main_pipeline: Vec<Transform>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Table {
    pub id: TId,

    /// Given name of this table.
    pub name: Option<String>,

    pub pipeline: Vec<Transform>,
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum Transform {
    From(TableRef, Vec<ColumnDef>),
    Derive(ColumnDef),
    Select(Vec<CId>),
    Filter(Expr),
    Aggregate(Vec<ColumnDef>),
    Sort(Vec<ColumnSort<CId>>),
    Take(Range<Expr>),
    Join {
        side: JoinSide,
        with: TableRef,
        filter: JoinFilter<CId>,
    },
    Unique,    
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
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
        Self { kind: WindowKind::Rows, range: Range::unbounded() }
    }
}