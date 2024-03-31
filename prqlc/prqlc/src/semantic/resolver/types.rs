use std::collections::HashSet;

use itertools::Itertools;

use crate::ast::{PrimitiveSet, Ty, TyFunc, TyKind, TyTupleField};
use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::Result;
use crate::{Error, Reason, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn infer_type(&self, expr: &Expr) -> Result<Option<Ty>> {
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

                for field in fields {
                    let ty = self.infer_type(field)?;

                    if field.flatten {
                        let ty = ty.clone().unwrap();
                        match ty.kind {
                            TyKind::Tuple(inner_fields) => {
                                ty_fields.extend(inner_fields);
                            }
                            _ => ty_fields.push(TyTupleField::Unpack(Some(ty))),
                        }

                        continue;
                    }

                    let name = field
                        .alias
                        .clone()
                        .or_else(|| self.infer_tuple_field_name(field));

                    ty_fields.push(TyTupleField::Single(name, ty));
                }
                ty_tuple_kind(ty_fields)
            }
            ExprKind::Array(items) => {
                let mut variants = Vec::with_capacity(items.len());
                for item in items {
                    let item_ty = self.infer_type(item)?;
                    if let Some(item_ty) = item_ty {
                        variants.push(item_ty);
                    }
                }
                // TODO
                let items_ty = variants.into_iter().next().unwrap();
                TyKind::Array(Box::new(items_ty))
            }

            ExprKind::All { within, except } => {
                let Some(within_ty) = self.infer_type(within)? else {
                    return Ok(None);
                };
                let Some(except_ty) = self.infer_type(except)? else {
                    return Ok(None);
                };
                let field_mask =
                    ty_tuple_exclusion(&within_ty, &except_ty, within.span, except.span)?;

                let new_fields = itertools::zip_eq(within_ty.kind.as_tuple().unwrap(), field_mask)
                    .filter(|(_, p)| *p)
                    .map(|(x, _)| x.clone())
                    .collect();
                TyKind::Tuple(new_fields)
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
        // if type is a generic, restrict the generic type arg
        match &expected.kind {
            TyKind::Ident(fq_ident) => {
                let Some(decl) = self.root_mod.module.get_mut(fq_ident) else {
                    return Ok(());
                };

                let DeclKind::GenericParam(inferred_type) = &mut decl.kind else {
                    return Ok(());
                };

                // TODO: check that we are not overriding here
                *inferred_type = Some((found.clone(), found.span));
                return Ok(());
            }
            TyKind::Array(expected_items) => {
                let TyKind::Array(found_items) = &mut found.kind else {
                    return Err(compose_type_error(found, expected, who).with_span(span));
                };

                return self.validate_type(found_items.as_mut(), expected_items, span, who);
            }
            TyKind::Tuple(expected_fields) => {
                let TyKind::Tuple(found_fields) = &mut found.kind else {
                    return Err(compose_type_error(found, expected, who).with_span(span));
                };

                for (e, f) in itertools::zip_eq(expected_fields, found_fields) {
                    if let (TyTupleField::Single(_, Some(e)), TyTupleField::Single(_, Some(f))) =
                        (e, f)
                    {
                        self.validate_type(f, e, span, who)?;
                    }
                }

                return Ok(());
            }
            TyKind::Primitive(e) => {
                if let TyKind::Primitive(f) = &found.kind {
                    if e == f {
                        return Ok(());
                    }
                }
            }
            _ => (),
        }

        Err(compose_type_error(found, expected, who).with_span(span))
    }

    pub fn finalize_generic_args(&self, mut ty: Ty) -> Result<Ty, Error> {
        ty.kind = match ty.kind {
            // the meaningful part
            TyKind::Ident(ref fq_ident) => {
                let decl = self.root_mod.module.get(fq_ident).unwrap();
                let DeclKind::GenericParam(inferred_type) = &decl.kind else {
                    return Ok(ty);
                };

                let Some((ty, _span)) = inferred_type.as_ref() else {
                    return Err(Error::new_simple(format!(
                        "cannot determine the type of generic arg {}",
                        fq_ident.name
                    ))
                    .with_span(ty.span));
                };
                return Ok(ty.clone());
            }

            // recurse into container types
            // this could probably be implemented with folding, but I'm lazy
            TyKind::Primitive(p) => TyKind::Primitive(p),
            TyKind::Tuple(fields) => {
                let mut new_fields = Vec::with_capacity(fields.len());

                for field in fields {
                    match field {
                        TyTupleField::Single(name, ty) => {
                            let ty = self.finalize_generic_args_opt(ty)?;
                            new_fields.push(TyTupleField::Single(name, ty));
                        }
                        TyTupleField::Unpack(ty) => {
                            let resolved = self.finalize_generic_args_opt(ty)?;
                            new_fields.push(TyTupleField::Unpack(resolved));
                        }
                    }
                }
                TyKind::Tuple(new_fields)
            }
            TyKind::Array(ty) => TyKind::Array(Box::new(self.finalize_generic_args(*ty)?)),
            TyKind::Function(func) => TyKind::Function(
                func.map(|f| -> Result<_, Error> {
                    Ok(TyFunc {
                        args: f
                            .args
                            .into_iter()
                            .map(|a| self.finalize_generic_args_opt(a))
                            .try_collect()?,
                        return_ty: Box::new(self.finalize_generic_args_opt(*f.return_ty)?),
                        name_hint: f.name_hint,
                    })
                })
                .transpose()?,
            ),
        };
        Ok(ty)
    }

    pub fn finalize_generic_args_opt(&self, ty: Option<Ty>) -> Result<Option<Ty>, Error> {
        ty.map(|x| self.finalize_generic_args(x)).transpose()
    }

    fn infer_tuple_field_name(&self, field: &Expr) -> Option<String> {
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

        let ty = base.ty.as_ref()?;
        self.apply_ty_tuple_indirection(ty, *pos as usize)
    }

    fn apply_ty_tuple_indirection(&self, ty: &Ty, pos: usize) -> Option<String> {
        match &ty.kind {
            TyKind::Tuple(fields) => {
                // this tuple might contain Unpacks (which affect positions of fields after them)
                // so we need to resolve this type full first.

                let unpack_pos = (fields.iter())
                    .position(|f| f.is_unpack())
                    .unwrap_or(fields.len());
                if pos < unpack_pos {
                    // unpacks don't interfere with preceding fields
                    let field = fields.get(pos)?;

                    field.as_single().unwrap().0.clone()
                } else {
                    let pos_within_unpack = pos - unpack_pos;

                    let unpack_ty = fields.get(unpack_pos)?.as_unpack().unwrap();
                    let unpack_ty = unpack_ty.as_ref().unwrap();

                    self.apply_ty_tuple_indirection(unpack_ty, pos_within_unpack)
                }
            }

            TyKind::Ident(fq_ident) => {
                let decl = self.root_mod.module.get(dbg!(fq_ident)).unwrap();
                let inferred_type = decl.kind.as_generic_param()?;
                let (inferred_type, _) = inferred_type.as_ref()?;

                self.apply_ty_tuple_indirection(inferred_type, pos)
            }

            _ => None,
        }
    }
}

pub fn ty_tuple_exclusion(
    within_ty: &Ty,
    except_ty: &Ty,
    within_span: Option<Span>,
    except_span: Option<Span>,
) -> Result<Vec<bool>> {
    let TyKind::Tuple(within_fields) = &within_ty.kind else {
        return Err(
            Error::new_simple("fields can only be excluded from a tuple")
                .push_hint(format!("got {}", write_ty(within_ty)))
                .with_span(within_span),
        );
    };
    let TyKind::Tuple(except_fields) = &except_ty.kind else {
        return Err(Error::new_simple("expected excluding fields to be a tuple")
            .push_hint(format!("got {}", write_ty(except_ty)))
            .with_span(except_span));
    };
    let except_fields: HashSet<&String> = except_fields
        .iter()
        .map(|field| match field {
            TyTupleField::Single(Some(name), _) => Ok(name),
            _ => Err(Error::new_simple("excluding fields need to be named")),
        })
        .collect::<Result<_>>()
        .with_span(except_span)?;

    let mut mask = Vec::new();
    for field in within_fields {
        mask.push(match &field {
            TyTupleField::Single(Some(name), _) => !except_fields.contains(&name),
            _ => true,
        });
    }
    Ok(mask)
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
                if fields.len() == 1 && fields[0].is_unpack() {
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
