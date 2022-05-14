use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::Node;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Native(NativeType),
    Parameterized(Box<Type>, Box<Node>),
    AnyOf(Vec<Type>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, strum::EnumString, strum::Display)]
pub enum NativeType {
    #[strum(to_string = "frame")]
    Frame,
    #[strum(to_string = "column")]
    Column,
    #[strum(to_string = "scalar")]
    Scalar, // TODO: ScalarKind enum
}

impl Type {
    pub const fn frame() -> Type {
        Type::Native(NativeType::Frame)
    }

    pub const fn column() -> Type {
        Type::Native(NativeType::Column)
    }
}

/// Implements a partial ordering or types:
/// - higher up are types that include many others (AnyOf, Any) and
/// - on the bottom are the atomic types (bool, string).
impl PartialOrd for Type {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Native(l0), Self::Native(r0)) => {
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
