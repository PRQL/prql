use std::fmt::{Debug, Display, Formatter, Result, Write};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::{Frame, Literal};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum SetExpr {
    /// Set of a built-in primitive type
    Primitive(TyLit),

    /// Set that contains only a literal value
    Singleton(Literal),

    /// Union of sets (sum)
    Union(Vec<(Option<String>, SetExpr)>),

    /// Set of tuples (product)
    Tuple(Vec<TupleElement>),

    /// Set of arrays
    Array(Box<SetExpr>),

    /// Set of sets.
    /// Used for exprs that can be converted to SetExpr and then used as a Ty.
    Set,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TupleElement {
    Single(Option<String>, SetExpr),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum Ty {
    /// Value is an element of this [SetExpr]
    SetExpr(SetExpr),

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

// Type of a function curry
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

            (Ty::SetExpr(left), Ty::SetExpr(right)) => left.is_superset_of(right),

            (Ty::Table(_), Ty::Table(_)) => true,

            (l, r) => l == r,
        }
    }
}

impl SetExpr {
    fn is_superset_of(&self, subset: &SetExpr) -> bool {
        match (self, subset) {
            // TODO: convert these to array
            (SetExpr::Primitive(TyLit::Column), SetExpr::Primitive(TyLit::Column)) => true,
            (SetExpr::Primitive(TyLit::Column), SetExpr::Primitive(_)) => true,
            (SetExpr::Primitive(_), SetExpr::Primitive(TyLit::Column)) => false,

            (SetExpr::Primitive(l0), SetExpr::Primitive(r0)) => l0 == r0,
            (SetExpr::Union(many), one) => many.iter().any(|(_, any)| any.is_superset_of(one)),
            (one, SetExpr::Union(many)) => many.iter().all(|(_, each)| one.is_superset_of(each)),

            (l, r) => l == r,
        }
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            Ty::SetExpr(lit) => write!(f, "{:}", lit),
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

impl Display for SetExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            SetExpr::Primitive(lit) => write!(f, "{:}", lit),
            SetExpr::Union(ts) => {
                for (i, (_, e)) in ts.iter().enumerate() {
                    write!(f, "{e}")?;
                    if i < ts.len() - 1 {
                        f.write_char('|')?;
                    }
                }
                Ok(())
            }
            SetExpr::Singleton(lit) => write!(f, "{:}", lit),
            SetExpr::Tuple(elements) => {
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
            SetExpr::Set => write!(f, "set"),
            SetExpr::Array(_) => todo!(),
        }
    }
}
