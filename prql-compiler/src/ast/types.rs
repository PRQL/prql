use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter, Result, Write};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::Frame;

#[derive(Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum Ty {
    Empty,
    Literal(TyLit),
    Named(String),
    Parameterized(Box<Ty>, Box<Ty>),
    AnyOf(Vec<Ty>),
    Function(TyFunc),

    Table(Frame),

    /// Means that we have no information about the type of the variable and
    /// that it should be inferred from other usages.
    Infer,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::EnumString, strum::Display,
)]
pub enum TyLit {
    #[strum(to_string = "column")]
    Column,
    #[strum(to_string = "scalar")]
    Scalar,
    #[strum(to_string = "integer")]
    Integer,
    #[strum(to_string = "float")]
    Float,
    #[strum(to_string = "bool")]
    Bool,
    #[strum(to_string = "string")]
    String,
    #[strum(to_string = "date")]
    Date,
    #[strum(to_string = "time")]
    Time,
    #[strum(to_string = "timestamp")]
    Timestamp,
}

// Type of a function curry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TyFunc {
    pub named: HashMap<String, Ty>,
    pub args: Vec<Ty>,
    pub return_ty: Box<Ty>,
}

impl Ty {
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
            // Not handled here. See type_resolver.
            (Ty::Infer, _) | (_, Ty::Infer) => None,

            (Ty::Literal(TyLit::Column), Ty::Literal(TyLit::Column)) => Some(Ordering::Equal),
            (Ty::Literal(TyLit::Column), Ty::Literal(_)) => Some(Ordering::Greater),
            (Ty::Literal(_), Ty::Literal(TyLit::Column)) => Some(Ordering::Less),

            (Ty::Literal(l0), Ty::Literal(r0)) => {
                if l0 == r0 {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Ty::AnyOf(many), one) => {
                if many.iter().any(|m| m >= one) {
                    Some(Ordering::Greater)
                } else {
                    None
                }
            }
            (one, Ty::AnyOf(many)) => {
                if many.iter().any(|m| m >= one) {
                    Some(Ordering::Less)
                } else {
                    None
                }
            }
            (Ty::Parameterized(l_ty, l_param), Ty::Parameterized(r_ty, r_param)) => {
                if l_ty == r_ty && l_param == r_param {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Ty::Parameterized(l_ty, _), r_ty) if **l_ty == *r_ty => Some(Ordering::Equal),
            (l_ty, Ty::Parameterized(r_ty, _)) if *l_ty == **r_ty => Some(Ordering::Equal),

            (Ty::Table(_), Ty::Table(_)) => Some(Ordering::Equal),

            (l, r) => {
                if l == r {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
        }
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            Ty::Empty => write!(f, "()"),
            Ty::Literal(lit) => write!(f, "{:}", lit),
            Ty::Named(name) => write!(f, "{:}", name),
            Ty::Parameterized(t, param) => {
                write!(f, "{t}<{}>", param)
            }
            Ty::AnyOf(ts) => {
                for (i, t) in ts.iter().enumerate() {
                    write!(f, "{t}")?;
                    if i < ts.len() - 1 {
                        f.write_char('|')?;
                    }
                }
                Ok(())
            }
            Ty::Table(frame) => write!(f, "table<{frame}>"),
            Ty::Infer => write!(f, "infer"),
            Ty::Function(func) => {
                write!(f, "func")?;

                for t in &func.args {
                    write!(f, " {t}")?;
                }
                write!(f, " -> {}", func.return_ty)?;
                Ok(())
            }
        }
    }
}

impl Debug for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(self, f)
    }
}
