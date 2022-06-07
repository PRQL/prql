use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::Node;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Ty {
    Literal(TyLit),
    Named(String),
    Parameterized(Box<Ty>, Box<Node>),
    AnyOf(Vec<Ty>),

    /// Means that we have no information about the type of the variable and
    /// that it should be inferred from other usages.
    Infer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, strum::EnumString, strum::Display)]
pub enum TyLit {
    #[strum(to_string = "table")]
    Table,
    #[strum(to_string = "column")]
    Column,
    #[strum(to_string = "scalar")]
    Scalar,
    #[strum(to_string = "integer")]
    Integer,
    #[strum(to_string = "float")]
    Float,
    #[strum(to_string = "boolean")]
    Boolean,
    #[strum(to_string = "string")]
    String,
    #[strum(to_string = "date")]
    Date,
    #[strum(to_string = "time")]
    Time,
    #[strum(to_string = "timestamp")]
    Timestamp,
}

impl Ty {
    pub const fn frame() -> Ty {
        Ty::Literal(TyLit::Table)
    }

    pub const fn column() -> Ty {
        Ty::Literal(TyLit::Column)
    }
}

impl From<TyLit> for Ty {
    fn from(lit: TyLit) -> Self {
        Ty::Literal(lit)
    }
}

impl Default for Ty {
    fn default() -> Self {
        Ty::Infer
    }
}

/// Implements a partial ordering or types:
/// - higher up are types that include many others (AnyOf, Any) and
/// - on the bottom are the atomic types (bool, string).
impl PartialOrd for Ty {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Literal(l0), Self::Literal(r0)) => {
                if l0 == r0 {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Self::Parameterized(l0, l1), Self::Parameterized(r0, r1)) => {
                if l0 == r0 && l1 == r1 {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Self::AnyOf(many), one) => {
                if many.iter().any(|m| m == one) {
                    Some(Ordering::Greater)
                } else {
                    None
                }
            }
            (one, Self::AnyOf(many)) => {
                if many.iter().any(|m| m == one) {
                    Some(Ordering::Less)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
