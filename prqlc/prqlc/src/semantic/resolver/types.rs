use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::codegen::{write_ty, write_ty_kind};
use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::*;
use crate::pr::{PrimitiveSet, Ty, TyFunc, TyKind, TyTupleField};
use crate::semantic::{NS_GENERIC, NS_LOCAL};
use crate::Result;
use crate::{Error, Reason, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    /// Visit a type in the main resolver pass. It will:
    /// - resolve [TyKind::Ident] to material types (expect for the ones that point to generic type arguments),
    /// - inline [TyTupleField::Unpack],
    /// - inline [TyKind::Exclude].
    // This function is named fold_type_actual, because fold_type must be in
    // expr.rs, where we implement PlFold.
    pub fn fold_type_actual(&mut self, ty: Ty) -> Result<Ty> {
        Ok(match ty.kind {
            TyKind::Ident(ident) => {
                let decl = self.get_ident(&ident).ok_or_else(|| {
                    Error::new_assert("cannot find type ident")
                        .push_hint(format!("ident={ident:?}"))
                })?;

                let mut fold_again = false;
                let ty = match &decl.kind {
                    DeclKind::Ty(ref_ty) => {
                        // materialize into the referred type
                        fold_again = true;
                        let inferred_name = if ident.starts_with_part(NS_GENERIC)
                            || ident.starts_with_part(NS_LOCAL)
                        {
                            None
                        } else {
                            Some(ident.name)
                        };
                        Ty {
                            kind: ref_ty.kind.clone(),
                            name: ref_ty.name.clone().or(inferred_name),
                            span: ty.span,
                        }
                    }

                    DeclKind::GenericParam(_) => {
                        // leave as an ident
                        Ty {
                            name: Some(ident.name.clone()),
                            kind: TyKind::Ident(ident),
                            ..ty
                        }
                    }

                    DeclKind::Unresolved(_) => {
                        return Err(Error::new_assert(format!(
                            "bad resolution order: unresolved {ident} while resolving {}",
                            self.debug_current_decl
                        ))
                        .with_span(ty.span))
                    }
                    _ => {
                        return Err(Error::new(Reason::Expected {
                            who: None,
                            expected: "a type".to_string(),
                            found: decl.to_string(),
                        })
                        .with_span(ty.span))
                    }
                };

                if fold_again {
                    self.fold_type_actual(ty)?
                } else {
                    ty
                }
            }
            TyKind::Tuple(fields) => Ty {
                kind: TyKind::Tuple(ty_fold_and_inline_tuple_fields(self, fields)?),
                ..ty
            },
            TyKind::Exclude { base, except } => {
                let base = self.fold_type(*base)?;
                let except = self.fold_type(*except)?;

                Ty {
                    kind: self.ty_tuple_exclusion(base, except)?,
                    ..ty
                }
            }
            _ => fold_type(self, ty)?,
        })
    }

    pub fn infer_type(&mut self, expr: &Expr) -> Result<Option<Ty>> {
        if let Some(ty) = &expr.ty {
            return Ok(Some(ty.clone()));
        }

        let kind = match &expr.kind {
            ExprKind::Literal(ref literal) => match literal {
                Literal::Null => return Ok(None), // TODO
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
                let items_ty = match variants.len() {
                    0 => {
                        // no items, so we must infer the type
                        let generic_ident = self.init_new_global_generic("A");
                        Ty::new(TyKind::Ident(generic_ident))
                    }
                    1 => {
                        // single item, use its type
                        variants.into_iter().exactly_one().unwrap()
                    }
                    2.. => {
                        // ideally, we would enforce that all of items have
                        // the same type, but currently we don't have a good
                        // strategy for dealing with nullable types, which
                        // causes problems here.
                        // HACK: use only the first type
                        variants.into_iter().next().unwrap()
                    }
                };
                TyKind::Array(Box::new(items_ty))
            }

            ExprKind::All { within, except } => {
                let Some(within_ty) = self.infer_type(within)? else {
                    return Ok(None);
                };
                let Some(except_ty) = self.infer_type(except)? else {
                    return Ok(None);
                };
                self.ty_tuple_exclusion(within_ty, except_ty)?
            }

            ExprKind::Case(cases) => {
                let case_tys: Vec<Option<Ty>> = cases
                    .iter()
                    .map(|c| self.infer_type(&c.value))
                    .try_collect()?;

                let Some(inferred_ty) = case_tys.iter().find_map(|x| x.as_ref()) else {
                    return Err(Error::new_simple(
                        "cannot infer type of any of the branches of this case statement",
                    )
                    .with_span(expr.span));
                };

                return Ok(Some(inferred_ty.clone()));
            }

            ExprKind::Func(func) => TyKind::Function(Some(TyFunc {
                params: func.params.iter().map(|p| p.ty.clone()).collect_vec(),
                return_ty: func
                    .return_ty
                    .clone()
                    .or_else(|| func.body.ty.clone())
                    .map(Box::new),
                generic_type_params: func.generic_type_params.clone(),
            })),

            _ => return Ok(None),
        };
        Ok(Some(Ty {
            kind,
            name: None,
            span: expr.span,
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
            // expected is none: there is no validation to be done and no generic to be inferred
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
        found: &Ty,
        expected: &Ty,
        span: Option<Span>,
        who: &F,
    ) -> Result<(), Error>
    where
        F: Fn() -> Option<String>,
    {
        match (&found.kind, &expected.kind) {
            // base case
            (TyKind::Primitive(f), TyKind::Primitive(e)) if e == f => Ok(()),

            // generics: infer
            (_, TyKind::Ident(expected_fq)) => {
                // if expected type is a generic, infer that it must be the found type
                self.infer_generic_as_ty(expected_fq, found.clone(), found.span)?;
                Ok(())
            }
            (TyKind::Ident(found_fq), _) => {
                // if found type is a generic, infer that it must be the expected type
                self.infer_generic_as_ty(found_fq, expected.clone(), span)?;
                Ok(())
            }

            // containers: recurse
            (TyKind::Array(found_items), TyKind::Array(expected_items)) => {
                // co-variant contained type
                self.validate_type(found_items, expected_items, span, who)
            }
            (TyKind::Tuple(found_fields), TyKind::Tuple(expected_fields)) => {
                // here we need to check that found tuple has all fields that are expected.

                // build index of found fields
                let found_types: HashMap<_, _> = found_fields
                    .iter()
                    .filter_map(|e| match e {
                        TyTupleField::Single(Some(n), ty) => Some((n, ty)),
                        TyTupleField::Single(None, _) => None,
                        TyTupleField::Unpack(_) => None, // handled later
                    })
                    .collect();

                let mut expected_but_not_found = Vec::new();
                for e_field in expected_fields {
                    match e_field {
                        TyTupleField::Single(Some(e_name), e_ty) => {
                            // when a named field is expected

                            // if it was found
                            if let Some(f_ty) = found_types.get(e_name) {
                                // check its type
                                if let Some((f_ty, e_ty)) = f_ty.as_ref().zip(e_ty.as_ref()) {
                                    // co-variant contained type
                                    self.validate_type(f_ty, e_ty, span, who)?;
                                }
                            } else {
                                expected_but_not_found.push(e_field);
                            }
                        }
                        TyTupleField::Single(None, _) => {
                            // TODO: positional expected fields
                        }
                        TyTupleField::Unpack(_) => {} // handled later
                    }
                }

                if !expected_but_not_found.is_empty() {
                    // not all fields were found

                    // try looking into the unpack
                    if let Some(found_unpack) = found_fields.last().and_then(|f| f.as_unpack()) {
                        if let Some(f_unpack_ty) = found_unpack {
                            let remaining = Ty::new(TyKind::Tuple(
                                expected_but_not_found.into_iter().cloned().collect_vec(),
                            ));
                            self.validate_type(&remaining, f_unpack_ty, span, who)?;
                        } else {
                            // we don't know the type of unpack, so we cannot fully check if it has the fields
                        }
                    } else {
                        // there is no unpack, not_found fields are an error
                        return Err(compose_type_error(found, expected, who).with_span(span));
                    }
                }

                // if there is an expected unpack, check it too
                if let Some(Some(e_unpack)) = expected_fields.last().and_then(|f| f.as_unpack()) {
                    self.validate_type(found, e_unpack, span, who)?;
                }

                Ok(())
            }
            (TyKind::Function(Some(f_func)), TyKind::Function(Some(e_func)))
                if f_func.params.len() == e_func.params.len() =>
            {
                for (f_arg, e_arg) in itertools::zip_eq(&f_func.params, &e_func.params) {
                    if let Some((f_arg, e_arg)) = Option::zip(f_arg.as_ref(), e_arg.as_ref()) {
                        // contra-variant contained types
                        self.validate_type(e_arg, f_arg, span, who)?;
                    }
                }

                // return types
                if let Some((f_ret, e_ret)) = Option::zip(
                    Option::as_ref(&f_func.return_ty),
                    Option::as_ref(&e_func.return_ty),
                ) {
                    // co-variant contained type
                    self.validate_type(f_ret, e_ret, span, who)?;
                }
                Ok(())
            }
            _ => Err(compose_type_error(found, expected, who).with_span(span)),
        }
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
                let decl = self.root_mod.module.get(fq_ident).unwrap();
                let inferred_type = decl.kind.as_generic_param()?;
                let (inferred_type, _) = inferred_type.as_ref()?;

                self.apply_ty_tuple_indirection(inferred_type, pos)
            }

            _ => None,
        }
    }

    /// Instantiate generic type parameters into generic type arguments.
    ///
    /// When resolving a type of reference to a variable, we cannot just use the type
    /// of the variable as the type of the reference. That's because the variable might contain
    /// generic type arguments that need to differ between references to the same variable.
    ///
    /// For example:
    /// ```prql
    /// let plus_one = func <T> x<T> -> <T> x + 1
    ///
    /// let a = plus_one 1
    /// let b = plus_one 1.5
    /// ```
    ///
    /// Here, the first reference to `plus_one` must resolve with T=int and the second with T=float.
    ///
    /// This struct makes sure that distinct instanced of T are created from generic type param T.
    pub fn instantiate_type(&mut self, ty: Ty, id: usize) -> Ty {
        let TyKind::Function(Some(ty_func)) = &ty.kind else {
            return ty;
        };
        if ty_func.generic_type_params.is_empty() {
            return ty;
        }
        let prev_scope = Ident::from_path(vec![NS_LOCAL]);
        let new_scope = Ident::from_path(vec![NS_GENERIC.to_string(), id.to_string()]);

        let mut ident_mapping: HashMap<Ident, Ty> =
            HashMap::with_capacity(ty_func.generic_type_params.len());

        for gtp in &ty_func.generic_type_params {
            let new_ident = new_scope.clone() + Ident::from_name(&gtp.name);

            let decl = Decl::from(DeclKind::GenericParam(
                gtp.bound.as_ref().map(|t| (t.clone(), None)),
            ));
            self.root_mod
                .module
                .insert(new_ident.clone(), decl)
                .unwrap();

            ident_mapping.insert(
                prev_scope.clone() + Ident::from_name(&gtp.name),
                Ty::new(TyKind::Ident(new_ident)),
            );
        }

        TypeReplacer::on_ty(ty, ident_mapping)
    }

    pub fn ty_tuple_exclusion(&self, base: Ty, except: Ty) -> Result<TyKind> {
        let mask = self.ty_tuple_exclusion_mask(&base, &except)?;

        if let Some(mask) = mask {
            let new_fields = itertools::zip_eq(base.kind.as_tuple().unwrap(), mask)
                .filter(|(_, p)| *p)
                .map(|(x, _)| x.clone())
                .collect();

            Ok(TyKind::Tuple(new_fields))
        } else {
            Ok(TyKind::Exclude {
                base: Box::new(base),
                except: Box::new(except),
            })
        }
    }

    /// Computes the "field mask", which is a vector of booleans indicating if a field of
    /// base tuple type should appear in the resulting type.
    ///
    /// Returns `None` if:
    /// - base or exclude is a generic type argument, or
    /// - either of the types contains Unpack.
    pub fn ty_tuple_exclusion_mask(&self, base: &Ty, except: &Ty) -> Result<Option<Vec<bool>>> {
        let within_fields = match &base.kind {
            TyKind::Tuple(f) => f,

            // this is a generic, exclusion cannot be inlined
            TyKind::Ident(_) => return Ok(None),

            _ => {
                return Err(
                    Error::new_simple("fields can only be excluded from a tuple")
                        .push_hint(format!("got {}", write_ty_kind(&base.kind)))
                        .with_span(base.span),
                )
            }
        };
        if within_fields.last().map_or(false, |f| f.is_unpack()) {
            return Ok(None);
        }

        let except_fields = match &except.kind {
            TyKind::Tuple(f) => f,

            // this is a generic, exclusion cannot be inlined
            TyKind::Ident(_) => return Ok(None),

            _ => {
                return Err(Error::new_simple("expected excluded fields to be a tuple")
                    .push_hint(format!("got {}", write_ty_kind(&except.kind)))
                    .with_span(except.span));
            }
        };
        if except_fields.last().map_or(false, |f| f.is_unpack()) {
            return Ok(None);
        }

        let except_fields: HashSet<&String> = except_fields
            .iter()
            .map(|field| match field {
                TyTupleField::Single(Some(name), _) => Ok(name),
                TyTupleField::Single(None, _) => {
                    Err(Error::new_simple("excluded fields must be named"))
                }
                _ => unreachable!(),
            })
            .collect::<Result<_>>()
            .with_span(except.span)?;

        let mut mask = Vec::new();
        for field in within_fields {
            mask.push(match &field {
                TyTupleField::Single(Some(name), _) => !except_fields.contains(&name),
                TyTupleField::Single(None, _) => true,

                TyTupleField::Unpack(_) => unreachable!(),
            });
        }
        Ok(Some(mask))
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

fn compose_type_error<F>(found_ty: &Ty, expected: &Ty, who: &F) -> Error
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
        let to_what = "in this function call?";

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

pub fn ty_fold_and_inline_tuple_fields<F: ?Sized + PlFold>(
    fold: &mut F,
    fields: Vec<TyTupleField>,
) -> Result<Vec<TyTupleField>> {
    let mut new_fields = Vec::new();
    for field in fields {
        match field {
            TyTupleField::Single(name, Some(ty)) => {
                // standard folding
                let ty = fold.fold_type(ty)?;
                new_fields.push(TyTupleField::Single(name, Some(ty)));
            }
            TyTupleField::Unpack(Some(ty)) => {
                let ty = fold.fold_type(ty)?;

                // inline unpack if it contains a tuple
                if let TyKind::Tuple(inner_fields) = ty.kind {
                    new_fields.extend(inner_fields);
                } else {
                    new_fields.push(TyTupleField::Unpack(Some(ty)));
                }
            }
            _ => {
                // standard folding
                new_fields.push(field);
            }
        }
    }
    Ok(new_fields)
}

/// Replaces references to generic type parameters with (partially) resolved argument types
/// and makes makes the type "human friendly".
pub struct TypePreviewer<'r> {
    resolver: &'r super::Resolver<'r>,
}

impl<'r> TypePreviewer<'r> {
    pub fn run(resolver: &'r super::Resolver<'r>, ty: Ty) -> Ty {
        TypePreviewer { resolver }.fold_type(ty).unwrap()
    }
}

impl PlFold for TypePreviewer<'_> {
    fn fold_type(&mut self, mut ty: Ty) -> Result<Ty> {
        ty.kind = match ty.kind {
            TyKind::Ident(fq_ident) => {
                let root_mod = &self.resolver.root_mod.module;
                let decl = root_mod.get(&fq_ident).unwrap();

                let candidate = decl.kind.as_generic_param().unwrap();

                if let Some((candidate, _)) = candidate {
                    let mut previewed = self.fold_type(candidate.clone()).unwrap();
                    if let TyKind::Tuple(fields) = &mut previewed.kind {
                        fields.push(TyTupleField::Unpack(None));
                    }

                    previewed.kind
                } else {
                    TyKind::Ident(Ident::from_name("?"))
                }
            }
            TyKind::Tuple(fields) => {
                let mut fields = ty_fold_and_inline_tuple_fields(self, fields)?;

                // clear types of fields that are just Ident("?")
                for field in &mut fields {
                    let ty = match field {
                        TyTupleField::Single(_, ty) => ty,
                        TyTupleField::Unpack(ty) => ty,
                    };
                    let is_unknown = ty
                        .as_ref()
                        .and_then(|t| t.kind.as_ident())
                        .map_or(false, |i| i.name == "?");
                    if is_unknown {
                        *ty = None
                    }
                }
                TyKind::Tuple(fields)
            }
            _ => return fold_type(self, ty),
        };
        Ok(ty)
    }
}

pub struct TypeReplacer {
    mapping: HashMap<Ident, Ty>,
}

impl TypeReplacer {
    pub fn on_ty(ty: Ty, mapping: HashMap<Ident, Ty>) -> Ty {
        TypeReplacer { mapping }.fold_type(ty).unwrap()
    }

    pub fn on_func(func: Func, mapping: HashMap<Ident, Ty>) -> Func {
        TypeReplacer { mapping }.fold_func(func).unwrap()
    }
}

impl PlFold for TypeReplacer {
    fn fold_type(&mut self, mut ty: Ty) -> Result<Ty> {
        ty.kind = match ty.kind {
            TyKind::Ident(ident) => {
                if let Some(new_ty) = self.mapping.get(&ident) {
                    return Ok(new_ty.clone());
                } else {
                    TyKind::Ident(ident)
                }
            }
            _ => return fold_type(self, ty),
        };
        Ok(ty)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ir::decl::RootModule;

    #[track_caller]
    fn validate_type(found: &str, expected: &str) -> crate::Result<()> {
        let mut root_mod = RootModule::default();
        let mut r = Resolver::new(&mut root_mod);

        let found = parse_ty(found);
        let expected = parse_ty(expected);

        r.validate_type(&found, &expected, None, &|| None)
    }

    #[track_caller]
    fn parse_ty(source: &str) -> Ty {
        let source = format!("type x = {source}");
        let stmts = crate::parser::parse_source(&source, 0).unwrap();
        let stmt = stmts.into_iter().next().unwrap();
        stmt.kind.into_type_def().unwrap().value.unwrap()
    }

    #[test]
    fn validate_type_00() {
        validate_type("{a = int, b = bool}", "{a = int}").unwrap();
    }

    #[test]
    fn validate_type_01() {
        // should fail because field b is expected, but not found
        validate_type("{a = int}", "{a = int, b = int}").unwrap_err();
    }

    #[test]
    fn validate_type_02() {
        validate_type(
            "{a = int, b = {b1 = int, b2 = bool}}",
            "{a = int, b = {b1 = int}}",
        )
        .unwrap();
    }

    #[test]
    fn validate_type_03() {
        // should fail because field b.b2 is expected, but not found
        validate_type(
            "{a = int, b = {b1 = int}}",
            "{a = int, b = {b1 = int, b2 = bool}}",
        )
        .unwrap_err();
    }

    #[test]
    fn validate_type_04() {
        // should fail because found b is bool instead of int
        validate_type("{a = int, b = bool}", "{a = int, b = int}").unwrap_err();
    }

    #[test]
    fn validate_type_05() {
        validate_type("{a = int, ..{b = bool}}", "{a = int, b = bool}").unwrap();
    }

    #[test]
    fn validate_type_06() {
        // should fail because found b is bool instead of int
        validate_type("{a = int, ..{b = bool}}", "{a = int, b = int}").unwrap_err();
    }

    #[test]
    fn validate_type_07() {
        validate_type("{a = int, b = bool}", "{a = int, ..{b = bool}}").unwrap();
    }

    #[test]
    fn validate_type_08() {
        // should fail because found b is bool instead of int
        validate_type("{a = int, b = bool}", "{a = int, ..{b = int}}").unwrap_err();
    }
}
