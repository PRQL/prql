use std::iter::zip;

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
    Array(Box<Ty>),

    /// Type of sets
    /// Used for expressions that can be converted to TypeExpr.
    Set,

    /// Type of functions with defined params and return types.
    Function(Option<TyFunc>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TupleField {
    /// Named tuple element.
    Single(Option<String>, Option<Ty>),

    /// Placeholder for possibly many elements.
    /// Means "and other unmentioned columns". Does not mean "all columns".
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
    pub fn relation(tuple_fields: Vec<TupleField>) -> Self {
        Ty {
            kind: TyKind::Array(Box::new(Ty {
                kind: TyKind::Tuple(tuple_fields),
                name: None,
            })),
            name: None,
        }
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
                matches!(elem.kind, TyKind::Tuple(_))
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

            (one, TyKind::Union(many)) => many
                .iter()
                .all(|(_, each)| one.is_super_type_of(&each.kind)),

            (TyKind::Union(many), one) => {
                many.iter().any(|(_, any)| any.kind.is_super_type_of(one))
            }

            (TyKind::Function(None), TyKind::Function(_)) => true,
            (TyKind::Function(Some(_)), TyKind::Function(None)) => true,
            (TyKind::Function(Some(sup)), TyKind::Function(Some(sub))) => {
                if is_not_super_type_of(sup.return_ty.as_ref(), sub.return_ty.as_ref()) {
                    return false;
                }
                if sup.args.len() != sub.args.len() {
                    return false;
                }
                for (sup_arg, sub_arg) in zip(&sup.args, &sub.args) {
                    if is_not_super_type_of(sup_arg, sub_arg) {
                        return false;
                    }
                }

                true
            }

            (l, r) => l == r,
        }
    }

    /// Analogous to [crate::ir::pl::Lineage::rename()]
    pub fn rename_relation(&mut self, alias: String) {
        if let TyKind::Array(items_ty) = self {
            items_ty.kind.rename_tuples(alias);
        }
    }

    fn rename_tuples(&mut self, alias: String) {
        self.flatten_tuples();

        if let TyKind::Tuple(fields) = self {
            let inner_fields = std::mem::take(fields);

            fields.push(TupleField::Single(
                Some(alias),
                Some(Ty {
                    kind: TyKind::Tuple(inner_fields),
                    name: None,
                }),
            ));
        }
    }

    fn flatten_tuples(&mut self) {
        if let TyKind::Tuple(fields) = self {
            let mut new_fields = Vec::new();

            for field in fields.drain(..) {
                let TupleField::Single(name, Some(ty)) = field else {
                    new_fields.push(field);
                    continue;
                };

                // recurse
                // let ty = ty.flatten_tuples();

                let TyKind::Tuple(inner_fields) = ty.kind else {
                    new_fields.push(TupleField::Single(name, Some(ty)));
                    continue;
                };
                new_fields.extend(inner_fields);
            }

            fields.extend(new_fields);
        }
    }
}

fn is_not_super_type_of(sup: &Option<Ty>, sub: &Option<Ty>) -> bool {
    if let Some(sub_ret) = sub {
        if let Some(sup_ret) = sup {
            if !sup_ret.is_super_type_of(sub_ret) {
                return true;
            }
        }
    }
    false
}
