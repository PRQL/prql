use std::fmt::{Debug, Display, Formatter, Result, Write};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::Literal;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TyKind {
    /// Type of a built-in primitive type
    Primitive(PrimitiveSet),

    /// Type that contains only a one value
    Singleton(Literal),

    /// Union of sets (sum)
    Union(Vec<(Option<String>, Ty)>),

    /// Type of tuples (product)
    Tuple(Vec<TupleField>),

    /// Type of arrays
    Array(Box<TyKind>),

    /// Type of sets
    /// Used for expressions that can be converted to TypeExpr.
    Set,

    /// Type of functions with defined params and return types.
    Function(TyFunc),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TupleField {
    // Named tuple element.
    Single(Option<String>, Option<Ty>),

    // Placeholder for possibly many elements.
    Wildcard(Option<Ty>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ty {
    pub kind: TyKind,

    /// Name inferred from the type declaration.
    pub name: Option<String>,
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
    pub args: Vec<Option<Ty>>,
    pub return_ty: Box<Option<Ty>>,
}

impl Ty {
    pub fn is_super_type_of(&self, subset: &Ty) -> bool {
        if self.is_relation() && subset.is_relation() {
            return true;
        }

        self.kind.is_super_type_of(&subset.kind)
    }

    pub fn is_sub_type_of_array(&self) -> bool {
        match &self.kind {
            TyKind::Array(_) => true,
            TyKind::Union(elements) => elements.iter().any(|(_, e)| e.is_sub_type_of_array()),
            _ => false,
        }
    }

    pub fn is_relation(&self) -> bool {
        match &self.kind {
            TyKind::Array(elem) => {
                matches!(elem.as_ref(), TyKind::Tuple(_))
            }
            _ => false,
        }
    }

    pub fn is_function(&self) -> bool {
        matches!(self.kind, TyKind::Function(_))
    }

    pub fn is_tuple(&self) -> bool {
        matches!(self.kind, TyKind::Tuple(_))
    }
}

impl TyKind {
    fn is_super_type_of(&self, subset: &TyKind) -> bool {
        match (self, subset) {
            (TyKind::Primitive(l0), TyKind::Primitive(r0)) => l0 == r0,
            (TyKind::Union(many), one) => {
                many.iter().any(|(_, any)| any.kind.is_super_type_of(one))
            }
            (one, TyKind::Union(many)) => many
                .iter()
                .all(|(_, each)| one.is_super_type_of(&each.kind)),

            (l, r) => l == r,
        }
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if let Some(name) = &self.name {
            return write!(f, "{}", name);
        }
        write!(f, "{:}", self.kind)
    }
}

pub fn display_ty(ty: Option<&Ty>) -> String {
    match ty {
        Some(ty) => ty.to_string(),
        None => "infer".to_string(),
    }
}

impl Display for TyKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            TyKind::Primitive(lit) => write!(f, "{:}", lit),
            TyKind::Union(ts) => {
                for (i, (_, e)) in ts.iter().enumerate() {
                    write!(f, "{e}")?;
                    if i < ts.len() - 1 {
                        f.write_char('|')?;
                    }
                }
                Ok(())
            }
            TyKind::Singleton(lit) => write!(f, "{:}", lit),
            TyKind::Tuple(elements) => {
                write!(f, "{{")?;
                for (index, e) in elements.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?
                    }
                    match e {
                        TupleField::Wildcard(generic_el) => match generic_el {
                            Some(el) => write!(f, "{el}..")?,
                            None => write!(f, "*..")?,
                        },
                        TupleField::Single(name, expr) => {
                            if let Some(name) = name {
                                write!(f, "{name} = ")?
                            }
                            if let Some(expr) = expr {
                                write!(f, "{expr}")?
                            } else {
                                write!(f, "?")?
                            }
                        }
                    }
                }
                write!(f, "}}")?;
                Ok(())
            }
            TyKind::Set => write!(f, "set"),
            TyKind::Array(elem) => write!(f, "{{{elem}}}"),
            TyKind::Function(func) => {
                write!(f, "func")?;

                for t in &func.args {
                    write!(f, " {}", display_ty(t.as_ref()))?;
                }
                write!(f, " -> {}", display_ty((*func.return_ty).as_ref()))?;
                Ok(())
            }
        }
    }
}
