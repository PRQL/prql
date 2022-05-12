use serde::{Deserialize, Serialize};

use super::Node;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Native(l0), Self::Native(r0)) => l0 == r0,
            (Self::Parameterized(l0, l1), Self::Parameterized(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::AnyOf(many), one) | (one, Self::AnyOf(many)) => many.iter().any(|m| m == one),
            _ => false,
        }
    }
}
