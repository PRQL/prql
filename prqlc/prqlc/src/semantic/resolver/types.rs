use itertools::Itertools;
use prqlc_ast::IndirectionKind;

use crate::ast::{PrimitiveSet, Ty, TyFunc, TyKind, TyTupleField};
use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::Result;
use crate::{Error, Reason, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn infer_type(expr: &Expr) -> Result<Option<Ty>> {
        if let Some(ty) = &expr.ty {
            return Ok(Some(ty.clone()));
        }

        let kind = match &expr.kind {
            ExprKind::Literal(ref literal) => match literal {
                Literal::Null => TyKind::Tuple(vec![]),
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

                    let name = field
                        .alias
                        .clone()
                        .or_else(|| infer_tuple_field_name(field));

                    ty_fields.push(TyTupleField::Single(name, ty));
                }

                if has_other {
                    ty_fields.push(TyTupleField::Wildcard(None));
                }
                ty_tuple_kind(ty_fields)
            }
            ExprKind::Array(items) => {
                let mut variants = Vec::with_capacity(items.len());
                for item in items {
                    let item_ty = Resolver::infer_type(item)?;
                    if let Some(item_ty) = item_ty {
                        variants.push(item_ty);
                    }
                }
                // TODO
                let items_ty = variants.into_iter().next().unwrap();
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
        let Some(expected) = expected else {
            // expected is none: there is no validation to be done
            return Ok(());
        };

        let Some(found_ty) = &mut found.ty else {
            // found is none: infer from expected
            found.ty = Some(expected.clone());
            return Ok(());
        };

        self.validate_type(found_ty, expected, found.span, who)
    }

    /// Validates that found node has expected type. Returns assumed type of the node.
    pub fn validate_type<F>(
        &mut self,
        found: &mut Ty,
        expected: &Ty,
        span: Option<Span>,
        who: &F,
    ) -> Result<(), Error>
    where
        F: Fn() -> Option<String>,
    {
        // A temporary hack for allowing calling window functions from within
        // aggregate and derive.
        if expected.kind.is_array() && !found.kind.is_function() {
            return Ok(());
        }

        // if type is a generic, restrict the generic type arg
        match &expected.kind {
            TyKind::GenericArg(generic_id) => {
                let inferred = self.generics.get_mut(generic_id).unwrap();
                inferred.push((found.clone(), found.span.clone()));

                return Ok(());
            }
            TyKind::Array(expected_items) => {
                let TyKind::Array(found_items) = &mut found.kind else {
                    return Err(compose_type_error(found, expected, who).with_span(span));
                };

                return self.validate_type(found_items.as_mut(), &expected_items, span, who);
            }
            TyKind::Tuple(expected_fields) => {
                let TyKind::Tuple(found_fields) = &mut found.kind else {
                    return Err(compose_type_error(found, expected, who).with_span(span));
                };

                for (e, f) in itertools::zip_eq(expected_fields, found_fields) {
                    match (e, f) {
                        (TyTupleField::Single(_, Some(e)), TyTupleField::Single(_, Some(f))) => {
                            self.validate_type(f, &e, span, who)?;
                        }
                        _ => {}
                    }
                }

                return Ok(());
            }
            _ => (),
        }

        Err(compose_type_error(found, expected, who).with_span(span))
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
            | DeclKind::TupleField(_)
            | DeclKind::Infer(_)
            | DeclKind::TableDecl(_)
            | DeclKind::Ty(_)
            | DeclKind::InstanceOf { .. }
            | DeclKind::Import(_)
            | DeclKind::Unresolved(_)
            | DeclKind::QueryDef(_) => {
                panic!("declaration {decl} is not able to have a type")
            }
        }
    }

    pub fn resolve_generic_args(&mut self, mut ty: Ty) -> Result<Ty, Error> {
        ty.kind = match ty.kind {
            // the meaningful part
            TyKind::GenericArg(id) => {
                let inferred_types = self.generics.remove(&id).unwrap();

                if inferred_types.len() != 1 {
                    return Err(Error::new_simple(
                        "cannot determine the type of generic arg",
                    ));
                }
                let (ty, _span) = inferred_types.into_iter().next().unwrap();
                return Ok(ty);
            }

            // recurse into container types
            // this could probably be implemented with folding, but I don't want another full fold impl
            TyKind::Tuple(fields) => TyKind::Tuple(
                fields
                    .into_iter()
                    .map(|field| -> Result<_, Error> {
                        Ok(match field {
                            TyTupleField::Single(name, ty) => {
                                TyTupleField::Single(name, self.resolve_generic_args_opt(ty)?)
                            }
                            TyTupleField::Wildcard(ty) => {
                                TyTupleField::Wildcard(self.resolve_generic_args_opt(ty)?)
                            }
                        })
                    })
                    .try_collect()?,
            ),
            TyKind::Array(ty) => TyKind::Array(Box::new(self.resolve_generic_args(*ty)?)),
            TyKind::Function(func) => TyKind::Function(
                func.map(|f| -> Result<_, Error> {
                    Ok(TyFunc {
                        args: f
                            .args
                            .into_iter()
                            .map(|a| self.resolve_generic_args_opt(a))
                            .try_collect()?,
                        return_ty: Box::new(self.resolve_generic_args_opt(*f.return_ty)?),
                        name_hint: f.name_hint,
                    })
                })
                .transpose()?,
            ),

            _ => ty.kind,
        };
        Ok(ty)
    }

    pub fn resolve_generic_args_opt(&mut self, ty: Option<Ty>) -> Result<Option<Ty>, Error> {
        ty.map(|x| self.resolve_generic_args(x)).transpose()
    }
}

fn infer_tuple_field_name(field: &Expr) -> Option<String> {
    // at this stage, this expr should already be fully resolved
    // this means that any indirections will be tuple positional
    // so we check for that and pull the name from the type of the base

    let ExprKind::Indirection {
        base,
        field: IndirectionKind::Position(pos),
    } = &field.kind
    else {
        return None;
    };

    let base_ty = base.ty.as_ref()?;
    let base_field = base_ty.kind.as_tuple()?.get(*pos as usize)?;

    base_field.as_single()?.0.clone()
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
        (TyKind::Primitive(_), _) => {}

        (TyKind::Tuple(tuple), TyKind::Tuple(sub_tuple)) => {
            for sub_field in sub_tuple {
                match sub_field {
                    TyTupleField::Single(sub_name, sub_ty) => {
                        if let Some(sub_name) = sub_name {
                            let existing = tuple
                                .iter_mut()
                                .filter_map(|x| x.as_single_mut())
                                .find(|f| f.0.as_ref() == Some(&sub_name));

                            if let Some((_, existing)) = existing {
                                restrict_type_opt(existing, sub_ty)
                            } else {
                                tuple.push(TyTupleField::Single(Some(sub_name), sub_ty));
                            }
                        } else {
                            // TODO: insert unnamed fields?
                        }
                    }
                    TyTupleField::Wildcard(_) => todo!("remove TupleField::Wildcard"),
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
