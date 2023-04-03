use std::fmt::{Debug, Display, Formatter, Result, Write};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::{Frame, Literal};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TypeExpr {
    /// Type of a built-in primitive type
    Primitive(TyLit),

    /// Type that contains only a literal value
    Singleton(Literal),

    /// Union of sets (sum)
    Union(Vec<(Option<String>, TypeExpr)>),

    /// Type of tuples (product)
    Tuple(Vec<TupleElement>),

    /// Type of arrays
    Array(Box<TypeExpr>),

    /// Type of sets.
    /// Used for exprs that can be converted to SetExpr and then used as a Ty.
    Type,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TupleElement {
    Single(Option<String>, TypeExpr),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum Ty {
    /// Value is an element of this [SetExpr]
    TypeExpr(TypeExpr),

    /// Value is a function described by [TyFunc]
    // TODO: convert into [Ty::Domain].
    Function(TyFunc),

    /// Special type for relations.
    // TODO: convert into [Ty::Domain].
    Table(Frame),

    /// Means that we have no information about the type of the variable and
    /// that it should be inferred from other usages.
    Infer,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::EnumString, strum::Display,
)]
pub enum TyLit {
    // TODO: convert to a named expression
    #[strum(to_string = "list")]
    List,
    // TODO: convert to a named expression
    #[strum(to_string = "column")]
    Column,
    // TODO: convert to a named expression
    #[strum(to_string = "scalar")]
    Scalar,
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
    pub args: Vec<Ty>,
    pub return_ty: Box<Ty>,
}

impl Ty {
    pub fn is_superset_of(&self, subset: &Ty) -> bool {
        match (self, subset) {
            // Not handled here. See type_resolver.
            (Ty::Infer, _) | (_, Ty::Infer) => false,

            (Ty::TypeExpr(left), Ty::TypeExpr(right)) => left.is_superset_of(right),

            (Ty::Table(_), Ty::Table(_)) => true,

            (l, r) => l == r,
        }
    }
}

impl TypeExpr {
    fn is_superset_of(&self, subset: &TypeExpr) -> bool {
        match (self, subset) {
            // TODO: convert these to array
            (TypeExpr::Primitive(TyLit::Column), TypeExpr::Primitive(TyLit::Column)) => true,
            (TypeExpr::Primitive(TyLit::Column), TypeExpr::Primitive(_)) => true,
            (TypeExpr::Primitive(_), TypeExpr::Primitive(TyLit::Column)) => false,

            (TypeExpr::Primitive(l0), TypeExpr::Primitive(r0)) => l0 == r0,
            (TypeExpr::Union(many), one) => many.iter().any(|(_, any)| any.is_superset_of(one)),
            (one, TypeExpr::Union(many)) => many.iter().all(|(_, each)| one.is_superset_of(each)),

            (l, r) => l == r,
        }
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            Ty::TypeExpr(lit) => write!(f, "{:}", lit),
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
                for e in elements {
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
                    write!(f, ",")?
                }
                Ok(())
            }
            TypeExpr::Type => write!(f, "set"),
            TypeExpr::Array(_) => todo!(),
        }
    }
}
