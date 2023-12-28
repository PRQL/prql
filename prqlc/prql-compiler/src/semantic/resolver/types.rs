use std::collections::HashMap;
use std::iter::zip;

use anyhow::Result;
use itertools::Itertools;
use prqlc_ast::{PrimitiveSet, TupleField, Ty, TyKind};

use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;

use crate::{Error, Reason, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn infer_type(expr: &Expr) -> Result<Option<Ty>> {
        if let Some(ty) = &expr.ty {
            return Ok(Some(ty.clone()));
        }

        let kind = match &expr.kind {
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
            ExprKind::Tuple(fields) => {
                let mut ty_fields: Vec<TupleField> = Vec::with_capacity(fields.len());
                let has_other = false;

                for field in fields {
                    let ty = Resolver::infer_type(field)?;

                    if field.flatten {
                        if let Some(fields) = ty.as_ref().and_then(|x| x.kind.as_tuple()) {
                            ty_fields.extend(fields.iter().cloned());
                            continue;
                        }
                    }

                    // TODO: move this into de-sugar stage (expand PL)
                    // TODO: this will not infer nested namespaces
                    let name = field
                        .alias
                        .clone()
                        .or_else(|| field.kind.as_ident().map(|i| i.name.clone()));

                    ty_fields.push(TupleField::Single(name, ty));
                }

                if has_other {
                    ty_fields.push(TupleField::Wildcard(None));
                }
                ty_tuple_kind(ty_fields)
            }
            ExprKind::Array(items) => {
                let mut variants = Vec::with_capacity(items.len());
                for item in items {
                    let item_ty = Resolver::infer_type(item)?;
                    if let Some(item_ty) = item_ty {
                        variants.push((None, item_ty));
                    }
                }
                let items_ty = Ty::new(TyKind::Union(variants));
                let items_ty = normalize_type(items_ty);
                TyKind::Array(Box::new(items_ty))
            }

            ExprKind::All { within, except } => {
                let base = Box::new(Resolver::infer_type(within)?.unwrap());
                let exclude = Box::new(Resolver::infer_type(except)?.unwrap());

                normalize_type(Ty::new(TyKind::Difference { base, exclude })).kind
            }

            _ => return Ok(None),
        };
        Ok(Some(Ty {
            kind,
            name: None,
            span: None,
        }))
    }

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

        // A temporary hack for allowing calling window functions from within
        // aggregate and derive.
        if expected.kind.is_array() && !found.kind.is_function() {
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

pub fn ty_tuple_kind(fields: Vec<TupleField>) -> TyKind {
    let mut res: Vec<TupleField> = Vec::with_capacity(fields.len());
    for field in fields {
        if let TupleField::Single(name, _) = &field {
            // remove names from previous fields with the same name
            if name.is_some() {
                for f in res.iter_mut() {
                    if f.as_single().and_then(|x| x.0.as_ref()) == name.as_ref() {
                        *f.as_single_mut().unwrap().0 = None;
                    }
                }
            }
        }
        res.push(field);
    }
    TyKind::Tuple(res)
}

/// Sink type difference operators down in the type expression,
/// float unions operators up, simplify type expression.
///
/// For more info, read web/book/src/reference/spec/type-system.md
pub(crate) fn normalize_type(ty: Ty) -> Ty {
    match ty.kind {
        TyKind::Union(variants) => {
            // A | () = A
            // A | A | B = A | B
            let mut res: Vec<(_, Ty)> = Vec::with_capacity(variants.len());

            let mut array_variants = Vec::new();
            let mut tuple_variants = Vec::new();

            for (variant_name, variant_ty) in variants {
                let variant_ty = normalize_type(variant_ty);

                // skip never
                if variant_ty.is_never() {
                    continue;
                }

                // handle array variants separately
                if let TyKind::Array(item) = variant_ty.kind {
                    array_variants.push((None, *item));
                    continue;
                }
                // handle tuple variants separately
                if let TyKind::Tuple(fields) = variant_ty.kind {
                    tuple_variants.push(fields);
                    continue;
                }

                // skip duplicates
                for (_, ty) in &res {
                    let intersection = type_intersection(ty.clone(), variant_ty.clone());
                    if &intersection == ty {
                        // this type is already fully included by another type
                        continue;
                    }
                }

                res.push((variant_name, variant_ty));
            }

            match array_variants.len() {
                2.. => {
                    let item_ty = Ty::new(TyKind::Union(array_variants));
                    res.push((None, Ty::new(TyKind::Array(Box::new(item_ty)))));
                }
                1 => {
                    let item_ty = array_variants.into_iter().next().unwrap().1;
                    res.push((None, Ty::new(TyKind::Array(Box::new(item_ty)))));
                }
                _ => {}
            }

            match tuple_variants.len() {
                2.. => {
                    res.push((None, union_of_tuples(tuple_variants)));
                }
                1 => {
                    let fields = tuple_variants.into_iter().next().unwrap();
                    res.push((None, Ty::new(TyKind::Tuple(fields))));
                }
                _ => {}
            }

            if res.len() == 1 {
                res.into_iter().next().unwrap().1
            } else {
                Ty {
                    kind: TyKind::Union(res),
                    ..ty
                }
            }
        }

        TyKind::Difference { base, exclude } => {
            let (base, exclude) = match (*base, *exclude) {
                // (A | B) - C = (A - C) | (B - C)
                (
                    Ty {
                        kind: TyKind::Union(variants),
                        name,
                        span,
                    },
                    c,
                ) => {
                    let kind = TyKind::Union(
                        variants
                            .into_iter()
                            .map(|(name, ty)| {
                                (
                                    name,
                                    Ty::new(TyKind::Difference {
                                        base: Box::new(ty),
                                        exclude: Box::new(c.clone()),
                                    }),
                                )
                            })
                            .collect(),
                    );
                    return normalize_type(Ty { kind, name, span });
                }
                // (A - B) - C = A - (B | C)
                (
                    Ty {
                        kind:
                            TyKind::Difference {
                                base: a,
                                exclude: b,
                            },
                        ..
                    },
                    c,
                ) => {
                    let kind = TyKind::Difference {
                        base: a,
                        exclude: Box::new(union_and_flatten(*b, c)),
                    };
                    return normalize_type(Ty { kind, ..ty });
                }

                // A - (B - C) =
                // = A & not (B & not C)
                // = A & (not B | C)
                // = (A & not B) | (A & C)
                // = (A - B) | (A & C)
                (
                    a,
                    Ty {
                        kind:
                            TyKind::Difference {
                                base: b,
                                exclude: c,
                            },
                        ..
                    },
                ) => {
                    let first = Ty::new(TyKind::Difference {
                        base: Box::new(a.clone()),
                        exclude: b,
                    });
                    let second = type_intersection(a, *c);
                    let kind = TyKind::Union(vec![(None, first), (None, second)]);
                    return normalize_type(Ty { kind, ..ty });
                }

                // [A] - [B] = [A - B]
                (
                    Ty {
                        kind: TyKind::Array(base),
                        ..
                    },
                    Ty {
                        kind: TyKind::Array(exclude),
                        ..
                    },
                ) => {
                    let item = Ty::new(TyKind::Difference { base, exclude });
                    let kind = TyKind::Array(Box::new(item));
                    return normalize_type(Ty { kind, ..ty });
                }
                // [A] - non-array = [A]
                (
                    Ty {
                        kind: TyKind::Array(item),
                        ..
                    },
                    _,
                ) => {
                    return normalize_type(Ty {
                        kind: TyKind::Array(item),
                        ..ty
                    });
                }
                // non-array - [B] = non-array
                (
                    base,
                    Ty {
                        kind: TyKind::Array(_),
                        ..
                    },
                ) => {
                    return normalize_type(base);
                }

                // {A, B} - {C, D} = {A - C, B - D}
                (
                    Ty {
                        kind: TyKind::Tuple(base_fields),
                        ..
                    },
                    Ty {
                        kind: TyKind::Tuple(exclude_fields),
                        ..
                    },
                ) => {
                    let exclude_fields: HashMap<&String, &Option<Ty>> = exclude_fields
                        .iter()
                        .flat_map(|field| match field {
                            TupleField::Single(Some(name), ty) => Some((name, ty)),
                            _ => None,
                        })
                        .collect();

                    let mut res = Vec::new();
                    for field in base_fields {
                        // TODO: this whole block should be redone - I'm not sure it fully correct.
                        match field {
                            TupleField::Single(Some(name), Some(ty)) => {
                                if let Some(right_field) = exclude_fields.get(&name) {
                                    let right_tuple =
                                        right_field.as_ref().map_or(false, |x| x.kind.is_tuple());

                                    if right_tuple {
                                        // recursively erase selection
                                        let ty = Ty::new(TyKind::Difference {
                                            base: Box::new(ty),
                                            exclude: Box::new((*right_field).clone().unwrap()),
                                        });
                                        let ty = normalize_type(ty);
                                        res.push(TupleField::Single(Some(name), Some(ty)))
                                    } else {
                                        // erase completely
                                    }
                                } else {
                                    res.push(TupleField::Single(Some(name), Some(ty)))
                                }
                            }
                            TupleField::Single(Some(name), None) => {
                                if exclude_fields.get(&name).is_some() {
                                    // TODO: I'm not sure what should happen in this case
                                    continue;
                                } else {
                                    res.push(TupleField::Single(Some(name), None))
                                }
                            }
                            TupleField::Single(None, ty) => {
                                res.push(TupleField::Single(None, ty));
                            }
                            TupleField::Wildcard(_) => res.push(field),
                        }
                    }
                    return Ty {
                        kind: TyKind::Tuple(res),
                        ..ty
                    };
                }

                // noop
                (a, b) => (a, b),
            };

            let base = Box::new(normalize_type(base));
            let exclude = Box::new(normalize_type(exclude));

            // A - (A | B) = ()
            if let TyKind::Union(excluded) = &exclude.kind {
                for (_, e) in excluded {
                    if base.as_ref() == e {
                        return Ty::never();
                    }
                }
            }
            let kind = TyKind::Difference { base, exclude };
            Ty { kind, ..ty }
        }

        kind => Ty { kind, ..ty },
    }
}

fn union_of_tuples(tuple_variants: Vec<Vec<TupleField>>) -> Ty {
    let mut fields = Vec::<TupleField>::new();
    let mut has_wildcard = false;

    for tuple_variant in tuple_variants {
        for field in tuple_variant {
            match field {
                TupleField::Single(Some(name), ty) => {
                    // find by name
                    let existing = fields.iter_mut().find_map(|f| match f {
                        TupleField::Single(n, t) if n.as_ref() == Some(&name) => Some(t),
                        _ => None,
                    });
                    if let Some(existing) = existing {
                        // union with the existing
                        *existing = maybe_union(existing.take(), ty);
                    } else {
                        // push
                        fields.push(TupleField::Single(Some(name), ty));
                    }
                }
                TupleField::Single(None, ty) => {
                    // push
                    fields.push(TupleField::Single(None, ty));
                }
                TupleField::Wildcard(_) => has_wildcard = true,
            }
        }
    }
    if has_wildcard {
        fields.push(TupleField::Wildcard(None));
    }
    Ty::new(TyKind::Tuple(fields))
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
        if ty.name.is_none() {
            if let TyKind::Tuple(fields) = &ty.kind {
                if fields.len() == 1 && fields[0].is_wildcard() {
                    return "a tuple".to_string();
                }
            }
        }
        format!("type `{}`", write_ty(ty))
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

    if found_ty.kind.is_function() && !expected.kind.is_function() {
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

    if is_join && found_ty.kind.is_tuple() && !expected.kind.is_tuple() {
        e = e.push_hint("Try using `(...)` instead of `{...}`");
    }

    if let Some(expected_name) = &expected.name {
        let expanded = write_ty_kind(&expected.kind);
        e = e.push_hint(format!("Type `{expected_name}` expands to `{expanded}`"));
    }
    e
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
    let array = TyKind::Array(Box::new(Ty::new(TyKind::Any)));
    is_super_type_of_kind(&array, &ty.kind)
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
    match (a.kind, b.kind) {
        (TyKind::Any, b_kind) => Ty { kind: b_kind, ..b },
        (a_kind, TyKind::Any) => Ty { kind: a_kind, ..a },

        // union
        (TyKind::Union(a_variants), b_kind) => {
            let b = Ty { kind: b_kind, ..b };
            type_intersection_with_union(a_variants, b)
        }
        (a_kind, TyKind::Union(b_variants)) => {
            let a = Ty { kind: a_kind, ..a };
            type_intersection_with_union(b_variants, a)
        }

        // difference
        (TyKind::Difference { base, exclude }, b_kind) => {
            let b = Ty { kind: b_kind, ..b };
            let base = Box::new(type_intersection(*base, b));
            Ty::new(TyKind::Difference { base, exclude })
        }
        (a_kind, TyKind::Difference { base, exclude }) => {
            let a = Ty { kind: a_kind, ..a };
            let base = Box::new(type_intersection(a, *base));
            Ty::new(TyKind::Difference { base, exclude })
        }

        (a_kind, b_kind) if a_kind == b_kind => Ty { kind: a_kind, ..a },

        // tuple
        (TyKind::Tuple(a_fields), TyKind::Tuple(b_fields)) => {
            type_intersection_of_tuples(a_fields, b_fields)
        }

        // array
        (TyKind::Array(a), TyKind::Array(b)) => {
            Ty::new(TyKind::Array(Box::new(type_intersection(*a, *b))))
        }

        _ => Ty::never(),
    }
}
fn type_intersection_with_union(variants: Vec<(Option<String>, Ty)>, b: Ty) -> Ty {
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
                    (Some(a), Some(b)) if a == b => Some(a),
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

/// Converts:
/// - A, B into A | B and
/// - A, B | C into A | B | C and
/// - A | B, C into A | B | C.
fn union_and_flatten(a: Ty, b: Ty) -> Ty {
    let mut variants = Vec::with_capacity(2);
    if let TyKind::Union(v) = a.kind {
        variants.extend(v)
    } else {
        variants.push((None, a));
    }
    if let TyKind::Union(v) = b.kind {
        variants.extend(v)
    } else {
        variants.push((None, b));
    }
    Ty::new(TyKind::Union(variants))
}

fn maybe_union(a: Option<Ty>, b: Option<Ty>) -> Option<Ty> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Ty::new(TyKind::Union(vec![(None, a), (None, b)]))),
        (None, x) | (x, None) => x,
    }
}
