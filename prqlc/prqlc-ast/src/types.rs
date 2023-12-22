use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;

use crate::{Ident, Span};

use super::Literal;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Ty {
    pub kind: TyKind,

    pub span: Option<Span>,

    /// Name inferred from the type declaration.
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner, AsRefStr)]
pub enum TyKind {
    /// Identifier that still needs to be resolved.
    Ident(Ident),

    /// Type of a built-in primitive type
    Primitive(PrimitiveSet),

    /// Type that contains only a one value
    Singleton(Literal),

    /// Union of sets (sum)
    Union(Vec<(Option<String>, Ty)>),

    /// Type of tuples (product)
    Tuple(Vec<TupleField>),

    /// Type of arrays
    Array(Box<Ty>),

    /// Type of functions with defined params and return types.
    Function(Option<TyFunc>),

    /// Type of every possible value. Super type of all other types.
    /// The breaker of chains. Mother of types.
    Any,

    /// Type that is the largest subtype of `base` while not a subtype of `exclude`.
    Difference { base: Box<Ty>, exclude: Box<Ty> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TupleField {
    /// Named tuple element.
    Single(Option<String>, Option<Ty>),

    /// Placeholder for possibly many elements.
    /// Means "and other unmentioned columns". Does not mean "all columns".
    Wildcard(Option<Ty>),
}

/// Built-in sets.
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::EnumString, strum::Display,
)]
pub enum PrimitiveSet {
    #[strum(to_string = "int")]
    Int,
    #[strum(to_string = "float")]
    Float,
    #[strum(to_string = "bool")]
    Bool,
    #[strum(to_string = "text")]
    Text,
    #[strum(to_string = "date")]
    Date,
    #[strum(to_string = "time")]
    Time,
    #[strum(to_string = "timestamp")]
    Timestamp,
}

// Type of a function
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TyFunc {
    pub name_hint: Option<Ident>,
    pub args: Vec<Option<Ty>>,
    pub return_ty: Box<Option<Ty>>,
}

impl Ty {
    pub fn new<K: Into<TyKind>>(kind: K) -> Ty {
        Ty {
            kind: kind.into(),
            span: None,
            name: None,
        }
    }

    pub fn relation(tuple_fields: Vec<TupleField>) -> Self {
        let tuple = Ty::new(TyKind::Tuple(tuple_fields));
        Ty::new(TyKind::Array(Box::new(tuple)))
    }

    pub fn never() -> Self {
        Ty::new(TyKind::Union(Vec::new()))
    }

    pub fn is_never(&self) -> bool {
        self.kind.as_union().map_or(false, |x| x.is_empty())
    }

    pub fn as_relation(&self) -> Option<&Vec<TupleField>> {
        self.kind.as_array()?.kind.as_tuple()
    }

    pub fn as_relation_mut(&mut self) -> Option<&mut Vec<TupleField>> {
        self.kind.as_array_mut()?.kind.as_tuple_mut()
    }

    pub fn into_relation(self) -> Option<Vec<TupleField>> {
        self.kind.into_array().ok()?.kind.into_tuple().ok()
    }

    pub fn is_relation(&self) -> bool {
        match &self.kind {
            TyKind::Array(elem) => {
                matches!(elem.kind, TyKind::Tuple(_))
            }
            _ => false,
        }
    }
}

impl TupleField {
    pub fn ty(&self) -> Option<&Ty> {
        match self {
            TupleField::Single(_, ty) => ty.as_ref(),
            TupleField::Wildcard(ty) => ty.as_ref(),
        }
    }
}

impl From<PrimitiveSet> for TyKind {
    fn from(value: PrimitiveSet) -> Self {
        TyKind::Primitive(value)
    }
}

impl From<TyFunc> for TyKind {
    fn from(value: TyFunc) -> Self {
        TyKind::Function(Some(value))
    }
}

impl From<Literal> for TyKind {
    fn from(value: Literal) -> Self {
        TyKind::Singleton(value)
    }
}

impl std::fmt::Debug for Ty {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
