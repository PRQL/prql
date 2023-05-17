use std::fmt::{Debug, Display, Formatter, Result, Write};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::{Frame, Literal};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TypeExpr {
    /// Type of a built-in primitive type
    Primitive(TyLit),

    /// Type that contains only a one value
    Singleton(Literal),

    /// Union of sets (sum)
    Union(Vec<(Option<String>, TypeExpr)>),

    /// Type of tuples (product)
    Tuple(Vec<TupleElement>),

    /// Type of arrays
    Array(Box<TypeExpr>),

    /// Type of sets
    /// Used for expressions that can be converted to TypeExpr.
    Set,

    /// Type of functions with defined params and return types.
    Function(TyFunc),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TupleElement {
    Single(Option<String>, TypeExpr),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ty {
    pub kind: TyKind,

    /// Name inferred from the type declaration.
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TyKind {
    /// Value is an element of this [TypeExpr]
    TypeExpr(TypeExpr),

    /// Special type for relations.
    // TODO: convert into [TypeExpr].
    Table(Frame),
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::EnumString, strum::Display,
)]
pub enum TyLit {
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
    pub fn is_superset_of(&self, subset: &Ty) -> bool {
        match (&self.kind, &subset.kind) {
            (TyKind::TypeExpr(TypeExpr::Array(_)), TyKind::Table(_)) => {
                // TODO: a temporary workaround that says "tables are subtypes of arrays"
                // so we can have a distinct type for tables (which should get merged into array of tuples)
                true
            }

            (TyKind::TypeExpr(left), TyKind::TypeExpr(right)) => left.is_superset_of(right),

            (TyKind::Table(_), TyKind::Table(_)) => true,

            (l, r) => l == r,
        }
    }

    pub fn is_array(&self) -> bool {
        match &self.kind {
            TyKind::TypeExpr(e) => e.is_array(),
            _ => false,
        }
    }

    pub fn is_table(&self) -> bool {
        match &self.kind {
            TyKind::Table(_) => true,
            TyKind::TypeExpr(TypeExpr::Array(elem)) => {
                matches!(elem.as_ref(), TypeExpr::Tuple(_))
            }
            _ => false,
        }
    }

    pub fn is_function(&self) -> bool {
        matches!(self.kind, TyKind::TypeExpr(TypeExpr::Function(_)))
    }
}

impl TypeExpr {
    fn is_superset_of(&self, subset: &TypeExpr) -> bool {
        match (self, subset) {
            (TypeExpr::Primitive(l0), TypeExpr::Primitive(r0)) => l0 == r0,
            (TypeExpr::Union(many), one) => many.iter().any(|(_, any)| any.is_superset_of(one)),
            (one, TypeExpr::Union(many)) => many.iter().all(|(_, each)| one.is_superset_of(each)),

            (l, r) => l == r,
        }
    }

    pub fn is_array(&self) -> bool {
        match self {
            TypeExpr::Array(_) => true,
            TypeExpr::Union(elements) => elements.iter().any(|(_, e)| e.is_array()),
            _ => false,
        }
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if let Some(name) = &self.name {
            return write!(f, "{}", name);
        }

        match &self.kind {
            TyKind::TypeExpr(ty_expr) => write!(f, "{:}", ty_expr),
            TyKind::Table(frame) => write!(f, "table<{frame}>"),
        }
    }
}

pub fn display_ty(ty: &Option<Ty>) -> String {
    match ty {
        Some(ty) => ty.to_string(),
        None => "infer".to_string(),
    }
}

impl Display for TypeExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            TypeExpr::Primitive(lit) => write!(f, "{:}", lit),
            TypeExpr::Union(ts) => {
                for (i, (_, e)) in ts.iter().enumerate() {
                    write!(f, "{e}")?;
                    if i < ts.len() - 1 {
                        f.write_char('|')?;
                    }
                }
                Ok(())
            }
            TypeExpr::Singleton(lit) => write!(f, "{:}", lit),
            TypeExpr::Tuple(elements) => {
                write!(f, "[")?;
                for (index, e) in elements.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?
                    }
                    match e {
                        TupleElement::Wildcard => {
                            write!(f, "*")?;
                        }
                        TupleElement::Single(name, expr) => {
                            if let Some(name) = name {
                                write!(f, "{name} = ")?
                            }
                            write!(f, "{expr}")?
                        }
                    }
                }
                write!(f, "]")?;
                Ok(())
            }
            TypeExpr::Set => write!(f, "set"),
            TypeExpr::Array(elem) => write!(f, "{{{elem}}}"),
            TypeExpr::Function(func) => {
                write!(f, "func")?;

                for t in &func.args {
                    write!(f, " {}", display_ty(t))?;
                }
                write!(f, " -> {}", display_ty(&func.return_ty))?;
                Ok(())
            }
        }
    }
}
