use std::iter::zip;

use anyhow::Result;
use itertools::Itertools;
use prqlc_ast::{PrimitiveSet, TupleField, Ty, TyKind};

use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;

use crate::semantic::write_pl;
use crate::{Error, Reason, WithErrorInfo};

use super::Resolver;

pub fn infer_type(node: &Expr) -> Result<Option<Ty>> {
    if let Some(ty) = &node.ty {
        return Ok(Some(ty.clone()));
    }

    let kind = match &node.kind {
        ExprKind::Literal(ref literal) => match literal {
            Literal::Null => TyKind::Singleton(Literal::Null),
            Literal::Integer(_) => TyKind::Primitive(PrimitiveSet::Int),
            Literal::Float(_) => TyKind::Primitive(PrimitiveSet::Float),
            Literal::Boolean(_) => TyKind::Primitive(PrimitiveSet::Bool),
            Literal::String(_) => TyKind::Primitive(PrimitiveSet::Text),
            Literal::Date(_) => TyKind::Primitive(PrimitiveSet::Date),
            Literal::Time(_) => TyKind::Primitive(PrimitiveSet::Time),
            Literal::Timestamp(_) => TyKind::Primitive(PrimitiveSet::Timestamp),
            Literal::ValueAndUnit(_) => return Ok(None), // TODO
        },

        ExprKind::Ident(_) | ExprKind::FuncCall(_) => return Ok(None),

        ExprKind::SString(_) => return Ok(None),
        ExprKind::FString(_) => TyKind::Primitive(PrimitiveSet::Text),

        ExprKind::TransformCall(_) => return Ok(None), // TODO
        ExprKind::Tuple(fields) => TyKind::Tuple(
            fields
                .iter()
                .map(|x| -> Result<_> {
                    let ty = infer_type(x)?;

                    Ok(TupleField::Single(x.alias.clone(), ty))
                })
                .try_collect()?,
        ),
        ExprKind::Array(items) => {
            let mut intersection = None;
            for item in items {
                let item_ty = infer_type(item)?;

                if let Some(item_ty) = item_ty {
                    if let Some(intersection) = &intersection {
                        if intersection != &item_ty {
                            // TODO: compute type intersection instead
                            return Ok(None);
                        }
                    } else {
                        intersection = Some(item_ty);
                    }
                }
            }
            let Some(items_ty) = intersection else {
                // TODO: return Array(Infer) instead of Infer
                return Ok(None);
            };
            TyKind::Array(Box::new(items_ty))
        }

        _ => return Ok(None),
    };
    Ok(Some(Ty {
        kind,
        name: None,
        span: None,
    }))
}

impl Resolver<'_> {
    /// Validates that found node has expected type. Returns assumed type of the node.
    pub fn validate_expr_type<F>(
        &mut self,
        found: &mut Expr,
        expected: Option<&Ty>,
        who: &F,
    ) -> Result<(), Error>
    where
        F: Fn() -> Option<String>,
    {
        if expected.is_none() {
            // expected is none: there is no validation to be done
            return Ok(());
        };

        let Some(found_ty) = &mut found.ty else {
            // found is none: infer from expected

            if found.lineage.is_none() && expected.unwrap().is_relation() {
                // special case: infer a table type
                // inferred tables are needed for s-strings that represent tables
                // similarly as normal table references, we want to be able to infer columns
                // of this table, which means it needs to be defined somewhere in the module structure.
                let frame =
                    self.declare_table_for_literal(found.id.unwrap(), None, found.alias.clone());

                // override the empty frame with frame of the new table
                found.lineage = Some(frame)
            }

            // base case: infer expected type
            found.ty = expected.cloned();

            return Ok(());
        };

        self.validate_type(found_ty, expected, who)
            .with_span(found.span)
    }

    /// Validates that found node has expected type. Returns assumed type of the node.
    pub fn validate_type<F>(
        &mut self,
        found: &mut Ty,
        expected: Option<&Ty>,
        who: &F,
    ) -> Result<(), Error>
    where
        F: Fn() -> Option<String>,
    {
        // infer
        let Some(expected) = expected else {
            // expected is none: there is no validation to be done
            return Ok(());
        };

        let expected_is_above = is_super_type_of(expected, found);
        if expected_is_above {
            return Ok(());
        }

        // if type is a generic, infer the constraint
        // TODO
        if false {
            // This case will happen when variable is for example an `int || timestamp`, but the
            // expected type is just `int`. If the variable allows it, we infer that the
            // variable is actually just `int`.

            // This if prevents variables of for example type `int || timestamp` to be inferred
            // as `text`.
            let is_found_above = is_super_type_of(found, expected);
            if is_found_above {
                restrict_type(found, expected.clone());

                // propagate the inference to table declarations
                // if let Some(instance_of) = &found.instance_of {
                //     self.push_type_info(instance_of, expected.clone())
                // }
                return Ok(());
            }
        }

        Err(compose_type_error(found, expected, who))
    }

    /// Saves information that declaration identified by `fq_ident` must be of type `sub_ty`.
    /// Param `sub_ty` must be a sub type of the current type of the declaration.
    #[allow(dead_code)]
    pub fn push_type_info(&mut self, fq_ident: &Ident, sub_ty: Ty) {
        let decl = self.root_mod.module.get_mut(fq_ident).unwrap();

        match &mut decl.kind {
            DeclKind::Expr(expr) => {
                restrict_type_opt(&mut expr.ty, Some(sub_ty));
            }

            DeclKind::Module(_)
            | DeclKind::LayeredModules(_)
            | DeclKind::Column(_)
            | DeclKind::Infer(_)
            | DeclKind::TableDecl(_)
            | DeclKind::Ty(_)
            | DeclKind::InstanceOf { .. }
            | DeclKind::QueryDef(_) => {
                panic!("declaration {decl} is not able to have a type")
            }
        }
    }
}

fn restrict_type_opt(ty: &mut Option<Ty>, sub_ty: Option<Ty>) {
    let Some(sub_ty) = sub_ty else {
        return;
    };
    if let Some(ty) = ty {
        restrict_type(ty, sub_ty)
    } else {
        *ty = Some(sub_ty);
    }
}

fn restrict_type(ty: &mut Ty, sub_ty: Ty) {
    match (&mut ty.kind, sub_ty.kind) {
        (TyKind::Any, sub) => ty.kind = sub,

        (TyKind::Union(variants), sub_kind) => {
            let sub_ty = Ty {
                kind: sub_kind,
                ..sub_ty
            };
            let drained = variants
                .drain(..)
                .filter(|(_, variant)| is_super_type_of(variant, &sub_ty))
                .map(|(name, mut ty)| {
                    restrict_type(&mut ty, sub_ty.clone());
                    (name, ty)
                })
                .collect_vec();
            variants.extend(drained);
        }

        (kind, TyKind::Union(sub_variants)) => {
            todo!("restrict {kind:?} to union of {sub_variants:?}")
        }

        (TyKind::Primitive(_), _) => {}

        (TyKind::Singleton(_), _) => {}

        (TyKind::Tuple(tuple), TyKind::Tuple(sub_tuple)) => {
            for sub_field in sub_tuple {
                match sub_field {
                    TupleField::Single(sub_name, sub_ty) => {
                        if let Some(sub_name) = sub_name {
                            let existing = tuple
                                .iter_mut()
                                .filter_map(|x| x.as_single_mut())
                                .find(|f| f.0.as_ref() == Some(&sub_name));

                            if let Some((_, existing)) = existing {
                                restrict_type_opt(existing, sub_ty)
                            } else {
                                tuple.push(TupleField::Single(Some(sub_name), sub_ty));
                            }
                        } else {
                            // TODO: insert unnamed fields?
                        }
                    }
                    TupleField::Wildcard(_) => todo!("remove TupleField::Wildcard"),
                }
            }
        }

        (TyKind::Array(ty), TyKind::Array(sub_ty)) => restrict_type(ty, *sub_ty),

        (TyKind::Function(ty), TyKind::Function(sub_ty)) => {
            if sub_ty.is_none() {
                return;
            }
            if ty.is_none() {
                *ty = sub_ty;
                return;
            }
            if let (Some(func), Some(sub_func)) = (ty, sub_ty) {
                todo!("restrict function {func:?} to function {sub_func:?}")
            }
        }

        _ => {
            panic!("trying to restrict a type with a non sub type")
        }
    }
}

fn compose_type_error<F>(found_ty: &mut Ty, expected: &Ty, who: &F) -> Error
where
    F: Fn() -> Option<String>,
{
    fn display_ty(ty: &Ty) -> String {
        if ty.name.is_none() && ty.is_tuple() {
            "a tuple".to_string()
        } else {
            format!("type `{}`", write_ty(ty))
        }
    }

    let who = who();
    let is_join = who
        .as_ref()
        .map(|x| x.contains("std.join"))
        .unwrap_or_default();

    let mut e = Error::new(Reason::Expected {
        who,
        expected: display_ty(expected),
        found: display_ty(found_ty),
    });

    if found_ty.is_function() && !expected.is_function() {
        let found = found_ty.kind.as_function().unwrap();
        let func_name = if let Some(func) = found {
            func.name_hint.as_ref()
        } else {
            None
        };
        let to_what = func_name
            .map(|n| format!("to function {n}"))
            .unwrap_or_else(|| "in this function call?".to_string());

        e = e.push_hint(format!("Have you forgotten an argument {to_what}?"));
    }

    if is_join && found_ty.is_tuple() && !expected.is_tuple() {
        e = e.push_hint("Try using `(...)` instead of `{...}`");
    }

    if let Some(expected_name) = &expected.name {
        let expanded = write_ty_kind(&expected.kind);
        e = e.push_hint(format!("Type `{expected_name}` expands to `{expanded}`"));
    }
    e
}

#[allow(dead_code)]
fn too_many_arguments(call: &FuncCall, expected_len: usize, passed_len: usize) -> Error {
    let err = Error::new(Reason::Expected {
        who: Some(write_pl(*call.name.clone())),
        expected: format!("{} arguments", expected_len),
        found: format!("{}", passed_len),
    });
    if passed_len >= 2 {
        err.push_hint(format!(
            "if you are calling a function, you may want to add parentheses `{} [{:?} {:?}]`",
            write_pl(*call.name.clone()),
            call.args[0],
            call.args[1]
        ))
    } else {
        err
    }
}

/// Analogous to [crate::ir::pl::Lineage::rename()]
pub fn rename_relation(ty_kind: &mut TyKind, alias: String) {
    if let TyKind::Array(items_ty) = ty_kind {
        rename_tuples(&mut items_ty.kind, alias);
    }
}

fn rename_tuples(ty_kind: &mut TyKind, alias: String) {
    flatten_tuples(ty_kind);

    if let TyKind::Tuple(fields) = ty_kind {
        let inner_fields = std::mem::take(fields);

        let ty = Ty::new(TyKind::Tuple(inner_fields));
        fields.push(TupleField::Single(Some(alias), Some(ty)));
    }
}

fn flatten_tuples(ty_kind: &mut TyKind) {
    if let TyKind::Tuple(fields) = ty_kind {
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

pub fn is_super_type_of(superset: &Ty, subset: &Ty) -> bool {
    if superset.is_relation() && subset.is_relation() {
        return true;
    }
    is_super_type_of_kind(&superset.kind, &subset.kind)
}

pub fn is_sub_type_of_array(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Array(_) => true,
        TyKind::Union(elements) => elements.iter().any(|(_, e)| is_sub_type_of_array(e)),
        _ => false,
    }
}

fn is_super_type_of_kind(superset: &TyKind, subset: &TyKind) -> bool {
    match (superset, subset) {
        (TyKind::Any, _) => true,
        (_, TyKind::Any) => false,

        (TyKind::Primitive(l0), TyKind::Primitive(r0)) => l0 == r0,

        (one, TyKind::Union(many)) => many
            .iter()
            .all(|(_, each)| is_super_type_of_kind(one, &each.kind)),

        (TyKind::Union(many), one) => many
            .iter()
            .any(|(_, any)| is_super_type_of_kind(&any.kind, one)),

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

        (TyKind::Array(sup), TyKind::Array(sub)) => is_super_type_of(sup, sub),

        (TyKind::Tuple(sup_tuple), TyKind::Tuple(sub_tuple)) => {
            let sup_has_wildcard = sup_tuple
                .iter()
                .any(|f| matches!(f, TupleField::Wildcard(_)));
            let sub_has_wildcard = sub_tuple
                .iter()
                .any(|f| matches!(f, TupleField::Wildcard(_)));

            let mut sup_fields = sup_tuple.iter().filter(|f| f.is_single());
            let mut sub_fields = sub_tuple.iter().filter(|f| f.is_single());

            loop {
                let sup = sup_fields.next();
                let sub = sub_fields.next();

                match (sup, sub) {
                    (Some(TupleField::Single(_, sup)), Some(TupleField::Single(_, sub))) => {
                        if is_not_super_type_of(sup, sub) {
                            return false;
                        }
                    }
                    (_, Some(_)) => {
                        if !sup_has_wildcard {
                            return false;
                        }
                    }
                    (Some(_), None) => {
                        if !sub_has_wildcard {
                            return false;
                        }
                    }
                    (None, None) => break,
                }
            }
            true
        }

        (l, r) => l == r,
    }
}

fn is_not_super_type_of(sup: &Option<Ty>, sub: &Option<Ty>) -> bool {
    if let Some(sub_ret) = sub {
        if let Some(sup_ret) = sup {
            if !is_super_type_of(sup_ret, sub_ret) {
                return true;
            }
        }
    }
    false
}

fn maybe_type_intersection(a: Option<Ty>, b: Option<Ty>) -> Option<Ty> {
    match (a, b) {
        (Some(a), Some(b)) => Some(type_intersection(a, b)),
        (x, None) | (None, x) => x,
    }
}

pub fn type_intersection(a: Ty, b: Ty) -> Ty {
    match (&a.kind, &b.kind) {
        (a_kind, b_kind) if a_kind == b_kind => a,

        (TyKind::Any, _) => b,
        (_, TyKind::Any) => a,

        (TyKind::Union(_), _) => type_intersection_with_union(a, b),
        (_, TyKind::Union(_)) => type_intersection_with_union(b, a),

        (TyKind::Tuple(_), TyKind::Tuple(_)) => {
            let a = a.kind.into_tuple().unwrap();
            let b = b.kind.into_tuple().unwrap();

            type_intersection_of_tuples(a, b)
        }

        (TyKind::Array(_), TyKind::Array(_)) => {
            let a = a.kind.into_array().unwrap();
            let b = b.kind.into_array().unwrap();
            Ty::new(TyKind::Array(Box::new(type_intersection(*a, *b))))
        }

        _ => Ty::never(),
    }
}
fn type_intersection_with_union(union: Ty, b: Ty) -> Ty {
    let variants = union.kind.into_union().unwrap();
    let variants = variants
        .into_iter()
        .map(|(name, variant)| {
            let inter = type_intersection(variant, b.clone());

            (name, inter)
        })
        .collect_vec();

    Ty::new(TyKind::Union(variants))
}

fn type_intersection_of_tuples(a: Vec<TupleField>, b: Vec<TupleField>) -> Ty {
    let a_has_other = a.iter().any(|f| f.is_wildcard());
    let b_has_other = b.iter().any(|f| f.is_wildcard());

    let mut a_fields = a.into_iter().filter_map(|f| f.into_single().ok());
    let mut b_fields = b.into_iter().filter_map(|f| f.into_single().ok());

    let mut fields = Vec::new();
    let mut has_other = false;
    loop {
        match (a_fields.next(), b_fields.next()) {
            (None, None) => break,
            (None, Some(b_field)) => {
                if !a_has_other {
                    return Ty::never();
                }
                has_other = true;
                fields.push(TupleField::Single(b_field.0, b_field.1));
            }
            (Some(a_field), None) => {
                if !b_has_other {
                    return Ty::never();
                }
                has_other = true;
                fields.push(TupleField::Single(a_field.0, a_field.1));
            }
            (Some((a_name, a_ty)), Some((b_name, b_ty))) => {
                let name = match (a_name, b_name) {
                    (None, None) | (Some(_), Some(_)) => None,
                    (None, Some(n)) | (Some(n), None) => Some(n),
                };
                let ty = maybe_type_intersection(a_ty, b_ty);

                fields.push(TupleField::Single(name, ty));
            }
        }
    }
    if has_other {
        fields.push(TupleField::Wildcard(None));
    }

    Ty::new(TyKind::Tuple(fields))
}
