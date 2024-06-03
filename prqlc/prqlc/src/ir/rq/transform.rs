use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::*;
use crate::ir::generic::ColumnSort;
use crate::ir::generic::WindowFrame;
use crate::ir::pl::JoinSide;

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum Transform {
    From(TableRef),
    Compute(Compute),
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
    Append(TableRef),
    Loop(Vec<Transform>),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Take {
    pub range: Range,
    pub partition: Vec<CId>,
    pub sort: Vec<ColumnSort<CId>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Compute {
    pub id: CId,
    pub expr: Expr,

    /// Parameters for window functions (or expressions).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<Window>,

    /// Must be set exactly on columns used in [Transform::Aggregate].
    #[serde(skip_serializing_if = "is_false", default)]
    pub is_aggregation: bool,
}

/// Transformation of a table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Window {
    pub frame: WindowFrame<Expr>,
    pub partition: Vec<CId>,
    pub sort: Vec<ColumnSort<CId>>,
}

fn is_false(b: &bool) -> bool {
    !b
}
