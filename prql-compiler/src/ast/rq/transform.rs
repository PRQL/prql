use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::ast::pl::JoinSide;
use crate::ast::pl::{ColumnSort, Range, WindowFrame};

use super::*;

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum Transform {
    From(TableRef),
    Compute(ColumnDecl),
    Select(Vec<CId>),
    Filter(Expr),
    Aggregate {
        partition: Vec<CId>,
        compute: Vec<CId>,
    },
    Sort(Vec<ColumnSort<CId>>),
    Take(Take),
    Join {
        side: JoinSide,
        with: TableRef,
        filter: Expr,
    },
    Unique,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Take {
    pub range: Range<Expr>,
    pub partition: Vec<CId>,
    pub sort: Vec<ColumnSort<CId>>,
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Window {
    pub frame: WindowFrame<Expr>,
    pub partition: Vec<CId>,
    pub sort: Vec<ColumnSort<CId>>,
}

/// Column declaration.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ColumnDecl {
    pub id: CId,
    pub kind: ColumnDeclKind,

    /// Paramaters for window functions (or expressions).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<Window>,

    /// Must be set exactly on columns used in [Transform::Aggregate].
    #[serde(skip_serializing_if = "is_false", default)]
    pub is_aggregation: bool,
}

/// Column declaration kind.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum ColumnDeclKind {
    /// A column that is out of scope of this query, referenced by name.
    /// Can only be used in [Relation::ExternRef].
    ExternRef(String),
    /// All columns of the relation.
    /// Can only be used in [Relation::ExternRef].
    Wildcard,
    /// A column computed using an expression.
    /// Can only be used in [Transform::Compute].
    Expr { name: Option<String>, expr: Expr },
}

impl ColumnDecl {
    pub fn get_name(&self) -> Option<&String> {
        match &self.kind {
            ColumnDeclKind::Expr { name, .. } => name.as_ref(),
            _ => None,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !b
}
