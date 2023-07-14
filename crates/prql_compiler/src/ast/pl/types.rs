use std::{collections::HashSet, iter::zip};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use super::{Ident, Literal};

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
    Function(TyFunc),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum TupleField {
    /// Named tuple element.
    Single(Option<String>, Option<Ty>),

    /// Placeholder for possibly many elements.
    /// Means "all columns". Does not mean "other unmentioned columns"
    All {
        ty: Option<Ty>,
        exclude: HashSet<Ident>,
    },
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Ty {
    pub kind: TyKind,

    /// Name inferred from the type declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    // Ids of the nodes that are the source for data of this type.
    // Can point to a table reference or a column expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<usize>,

    // Fully-qualified name of table that was instanced to produce this type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_of: Option<Ident>,
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
        let tuple = Ty::from(TyKind::Tuple(tuple_fields));
        Ty::from(TyKind::Array(Box::new(tuple)))
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

            (TyKind::Function(sup), TyKind::Function(sub)) => {
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

            (TyKind::Array(sup), TyKind::Array(sub)) => sup.is_super_type_of(sub),

            (TyKind::Tuple(sup_fields), TyKind::Tuple(_)) => {
                if sup_fields.iter().any(|x| x.is_all()) {
                    return true;
                }
                // TODO: compare fields one by one
                false
            }

            (l, r) => l == r,
        }
    }
}

impl Ty {
    /// Converts [{T1, x = T2, y = {T3, z = T4}}]
    /// into [{alias = {T1, x = T2, T3, z = T4}}]
    pub fn rename_relation(&mut self, alias: String) {
        if let TyKind::Array(items_ty) = &mut self.kind {
            items_ty.rename_tuples(alias);
        }
    }

    /// Converts {T1, x = T2, y = {T3, z = T4}}
    /// into {alias = {T1, x = T2, T3, z = T4}}
    fn rename_tuples(&mut self, alias: String) {
        self.flatten_tuples();

        if let TyKind::Tuple(fields) = &mut self.kind {
            let inner_fields = std::mem::take(fields);

            let ty = Ty {
                lineage: self.lineage,
                instance_of: self.instance_of.clone(),
                ..Ty::from(TyKind::Tuple(inner_fields))
            };
            fields.push(TupleField::Single(Some(alias), Some(ty)));
        }
    }

    /// Converts {y = {T3, z = T4}}
    /// into {T3, z = T4}]
    pub fn flatten_tuples(&mut self) {
        if let TyKind::Tuple(fields) = &mut self.kind {
            let mut new_fields = Vec::new();

            for field in fields.drain(..) {
                if let TupleField::Single(name, Some(ty)) = field {
                    // recurse
                    // let ty = ty.flatten_tuples();

                    if let TyKind::Tuple(inner_fields) = ty.kind {
                        new_fields.extend(inner_fields);

                        if self.lineage.is_none() {
                            self.lineage = ty.lineage
                        };
                        if self.instance_of.is_none() {
                            self.instance_of = ty.instance_of
                        };
                        continue;
                    }

                    new_fields.push(TupleField::Single(name, Some(ty)));
                    continue;
                }

                new_fields.push(field);
            }

            fields.extend(new_fields);
        }
    }
}

impl TupleField {
    pub fn ty(&self) -> Option<&Ty> {
        match self {
            TupleField::Single(_, ty) => ty.as_ref(),
            TupleField::All { ty, .. } => ty.as_ref(),
        }
    }
}

impl From<TyKind> for Ty {
    fn from(kind: TyKind) -> Ty {
        Ty {
            kind,
            name: None,
            lineage: None,
            instance_of: None,
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

impl std::fmt::Debug for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}
