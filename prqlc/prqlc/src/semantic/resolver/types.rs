use super::Resolver;
use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::pl::*;
use crate::pr::{PrimitiveSet, Ty, TyKind, TyTupleField};
use crate::Result;
use crate::{Error, Reason, WithErrorInfo};

impl Resolver<'_> {
    pub fn infer_type(expr: &Expr) -> Result<Option<Ty>> {
        if let Some(ty) = &expr.ty {
            return Ok(Some(ty.clone()));
        }

        let kind = match &expr.kind {
            ExprKind::Literal(ref literal) => match literal {
                Literal::Null => return Ok(None),
                Literal::Integer(_) => TyKind::Primitive(PrimitiveSet::Int),
                Literal::Float(_) => TyKind::Primitive(PrimitiveSet::Float),
                Literal::Boolean(_) => TyKind::Primitive(PrimitiveSet::Bool),
                Literal::String(_) => TyKind::Primitive(PrimitiveSet::Text),
                Literal::RawString(_) => TyKind::Primitive(PrimitiveSet::Text),
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
                let mut ty_fields: Vec<TyTupleField> = Vec::with_capacity(fields.len());
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

                    ty_fields.push(TyTupleField::Single(name, ty));
                }

                if has_other {
                    ty_fields.push(TyTupleField::Wildcard(None));
                }
                ty_tuple_kind(ty_fields)
            }
            ExprKind::Array(items) => {
                let mut item_tys = Vec::with_capacity(items.len());
                for item in items {
                    let item_ty = Resolver::infer_type(item)?;
                    if let Some(item_ty) = item_ty {
                        item_tys.push(item_ty);
                    }
                }
                // TODO verify that types of all items are the same
                let items_ty = item_tys.into_iter().next().map(Box::new);
                TyKind::Array(items_ty)
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
                // of this table, which means it needs to be defined somewhere
                // in the module structure.
                let frame = self.declare_table_for_literal(
                    found
                        .clone()
                        .id
                        // This is quite rare but possible with something like
                        // `a -> b` at the moment.
                        .ok_or_else(|| Error::new_bug(4280))?,
                    None,
                    found.alias.clone(),
                );

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

        Err(compose_type_error(found, expected, who))
    }
}

pub fn ty_tuple_kind(fields: Vec<TyTupleField>) -> TyKind {
    let mut res: Vec<TyTupleField> = Vec::with_capacity(fields.len());
    for field in fields {
        if let TyTupleField::Single(name, _) = &field {
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
    if let TyKind::Array(Some(items_ty)) = ty_kind {
        rename_tuples(&mut items_ty.kind, alias);
    }
}

fn rename_tuples(ty_kind: &mut TyKind, alias: String) {
    flatten_tuples(ty_kind);

    if let TyKind::Tuple(fields) = ty_kind {
        let inner_fields = std::mem::take(fields);

        let ty = Ty::new(TyKind::Tuple(inner_fields));
        fields.push(TyTupleField::Single(Some(alias), Some(ty)));
    }
}

fn flatten_tuples(ty_kind: &mut TyKind) {
    if let TyKind::Tuple(fields) = ty_kind {
        let mut new_fields = Vec::new();

        for field in fields.drain(..) {
            let TyTupleField::Single(name, Some(ty)) = field else {
                new_fields.push(field);
                continue;
            };

            // recurse
            // let ty = ty.flatten_tuples();

            let TyKind::Tuple(inner_fields) = ty.kind else {
                new_fields.push(TyTupleField::Single(name, Some(ty)));
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

pub fn is_super_type_of_opt(superset: Option<&Ty>, subset: Option<&Ty>) -> bool {
    let Some(subset) = subset else {
        return true;
    };
    let Some(superset) = superset else {
        return true;
    };
    is_super_type_of_kind(&superset.kind, &subset.kind)
}

pub fn is_sub_type_of_array(ty: &Ty) -> bool {
    let array = TyKind::Array(None);
    is_super_type_of_kind(&array, &ty.kind)
}

fn is_super_type_of_kind(superset: &TyKind, subset: &TyKind) -> bool {
    match (superset, subset) {
        (TyKind::Primitive(l0), TyKind::Primitive(r0)) => l0 == r0,

        (TyKind::Function(None), TyKind::Function(_)) => true,
        (TyKind::Function(Some(_)), TyKind::Function(None)) => true,
        (TyKind::Function(Some(sup)), TyKind::Function(Some(sub))) => {
            if is_not_super_type_of(sup.return_ty.as_deref(), sub.return_ty.as_deref()) {
                return false;
            }
            if sup.params.len() != sub.params.len() {
                return false;
            }
            for (sup_arg, sub_arg) in sup.params.iter().zip(&sub.params) {
                if is_not_super_type_of(sup_arg.as_ref(), sub_arg.as_ref()) {
                    return false;
                }
            }

            true
        }

        (TyKind::Array(sup), TyKind::Array(sub)) => {
            is_super_type_of_opt(sup.as_deref(), sub.as_deref())
        }

        (TyKind::Tuple(sup_tuple), TyKind::Tuple(sub_tuple)) => {
            let sup_has_wildcard = sup_tuple
                .iter()
                .any(|f| matches!(f, TyTupleField::Wildcard(_)));
            let sub_has_wildcard = sub_tuple
                .iter()
                .any(|f| matches!(f, TyTupleField::Wildcard(_)));

            let mut sup_fields = sup_tuple.iter().filter(|f| f.is_single());
            let mut sub_fields = sub_tuple.iter().filter(|f| f.is_single());

            loop {
                let sup = sup_fields.next();
                let sub = sub_fields.next();

                match (sup, sub) {
                    (Some(TyTupleField::Single(_, sup)), Some(TyTupleField::Single(_, sub))) => {
                        if is_not_super_type_of(sup.as_ref(), sub.as_ref()) {
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

fn is_not_super_type_of(sup: Option<&Ty>, sub: Option<&Ty>) -> bool {
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
        (a_kind, b_kind) if a_kind == b_kind => Ty { kind: a_kind, ..a },

        // tuple
        (TyKind::Tuple(a_fields), TyKind::Tuple(b_fields)) => {
            type_intersection_of_tuples(a_fields, b_fields)
        }

        // array
        (TyKind::Array(Some(a)), TyKind::Array(Some(b))) => {
            Ty::new(TyKind::Array(Some(Box::new(type_intersection(*a, *b)))))
        }

        _ => todo!(),
    }
}

fn type_intersection_of_tuples(a: Vec<TyTupleField>, b: Vec<TyTupleField>) -> Ty {
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
                    todo!();
                }
                has_other = true;
                fields.push(TyTupleField::Single(b_field.0, b_field.1));
            }
            (Some(a_field), None) => {
                if !b_has_other {
                    todo!();
                }
                has_other = true;
                fields.push(TyTupleField::Single(a_field.0, a_field.1));
            }
            (Some((a_name, a_ty)), Some((b_name, b_ty))) => {
                let name = match (a_name, b_name) {
                    (Some(a), Some(b)) if a == b => Some(a),
                    (None, None) | (Some(_), Some(_)) => None,
                    (None, Some(n)) | (Some(n), None) => Some(n),
                };
                let ty = maybe_type_intersection(a_ty, b_ty);

                fields.push(TyTupleField::Single(name, ty));
            }
        }
    }
    if has_other {
        fields.push(TyTupleField::Wildcard(None));
    }

    Ty::new(TyKind::Tuple(fields))
}
