use itertools::Itertools;

use crate::ast::{Ty, TyKind, TyTupleField};
use crate::codegen::write_ty;
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::semantic::{NS_LOCAL, NS_SELF, NS_STD, NS_THIS};
use crate::{Error, Reason, Result, Span, WithErrorInfo};

impl PlFold for super::Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_type(&mut self, ty: Ty) -> Result<Ty> {
        self.fold_type_actual(ty)
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        let value = match var_def.value {
            Some(value) if matches!(value.kind, ExprKind::Func(_)) => Some(value),
            Some(value) => Some(Box::new(self.fold_expr(*value)?)),
            None => None,
        };

        Ok(VarDef {
            name: var_def.name,
            value,
            ty: var_def.ty.map(|x| self.fold_type(x)).transpose()?,
        })
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if node.id.is_some() && !matches!(node.kind, ExprKind::Func(_)) {
            return Ok(node);
        }

        let id = self.id.gen();
        let alias = Box::new(node.alias.clone());
        let span = Box::new(node.span);

        if let Some(span) = *span {
            self.root_mod.span_map.insert(id, span);
        }

        log::trace!("folding expr {node:?}");

        let r = match node.kind {
            ExprKind::Ident(ident) => {
                let mut indirections = Vec::new();

                log::debug!("resolving ident {ident:?}...");
                let fq_ident = if ident.starts_with_part(NS_LOCAL) {
                    // resolve first 2 parts as ident and convert all other into indirections
                    let mut parts = ident.into_iter();
                    let local = parts.next().unwrap();
                    let name_for_lookup = parts.next().unwrap();
                    indirections = parts.collect();

                    let name_for_lookup = Ident::from_path(vec![local, name_for_lookup]);
                    self.resolve_ident(&name_for_lookup).with_span(node.span)?
                } else {
                    self.resolve_ident(&ident).with_span(node.span)?
                };
                let log_debug = !fq_ident.starts_with_part(NS_STD);
                if log_debug {
                    log::debug!("... resolved to {fq_ident:?}")
                }
                let entry = self.root_mod.module.get(&fq_ident).unwrap();
                if log_debug {
                    log::debug!("... which is {entry}");
                }

                // strip `._self` suffix
                let fq_ident = if fq_ident.name == NS_SELF {
                    fq_ident.pop().unwrap()
                } else {
                    fq_ident
                };

                let expr = match &entry.kind {
                    DeclKind::Variable(ty) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ty: ty.clone(),
                        ..node
                    },

                    DeclKind::TupleField => {
                        indirections.push(fq_ident.name);
                        Expr::new(ExprKind::Ident(Ident::from_path(fq_ident.path)))
                    }

                    DeclKind::TableDecl(_) => {
                        let ty = self.ty_of_table_decl(&fq_ident);

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(ty),
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => match &expr.kind {
                        ExprKind::Func(closure) => {
                            let closure = self.fold_function_types(closure.clone(), id)?;

                            let expr = Expr::new(ExprKind::Func(closure));

                            if self.in_func_call_name {
                                expr
                            } else {
                                self.fold_expr(expr)?
                            }
                        }
                        _ => self.fold_expr(expr.as_ref().clone())?,
                    },

                    DeclKind::Ty(_) => {
                        return Err(Error::new(Reason::Expected {
                            who: None,
                            expected: "a value".to_string(),
                            found: "a type".to_string(),
                        })
                        .with_span(*span));
                    }

                    DeclKind::Infer(_) => unreachable!(),
                    DeclKind::Unresolved(_) => {
                        return Err(Error::new_assert(format!(
                            "bad resolution order: unresolved {fq_ident} while resolving {}",
                            self.debug_current_decl
                        )));
                    }

                    _ => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ..node
                    },
                };

                if !indirections.is_empty() {
                    let mut expr = expr;
                    for indirection in indirections {
                        expr = Expr::new(ExprKind::Indirection {
                            base: Box::new(expr),
                            field: IndirectionKind::Name(indirection),
                        });
                    }
                    expr.flatten = node.flatten;
                    self.fold_expr(expr)?
                } else {
                    expr
                }
            }

            ExprKind::Indirection { base, field } => {
                let base = self.fold_expr(*base)?;

                let ty = base.ty.as_ref().unwrap();
                let (position, ty) = self.resolve_indirection(ty, &field, 0).with_span(*span)?;
                Expr {
                    ty,
                    kind: ExprKind::Indirection {
                        base: Box::new(base),
                        field: IndirectionKind::Position(position as i64),
                    },
                    ..node
                }
            }

            ExprKind::FuncCall(FuncCall { name, args, .. })
                if (name.kind.as_ident()).map_or(false, |i| i.to_string() == "std.not")
                    && matches!(args[0].kind, ExprKind::Tuple(_)) =>
            {
                let arg = args.into_iter().exactly_one().unwrap();
                self.resolve_column_exclusion(arg)?
            }

            ExprKind::FuncCall(FuncCall {
                name,
                args,
                named_args,
            }) => {
                // fold function name
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                let name = Box::new(self.fold_expr(*name)?);
                self.in_func_call_name = old;

                let func = name.try_cast(|n| n.into_func(), None, "a function")?;

                // fold function
                let func = self.apply_args_to_closure(func, args, named_args)?;
                self.fold_function(func, id, *span)?
            }

            ExprKind::Func(closure) => self.fold_function(closure, id, *span)?,

            ExprKind::Tuple(exprs) => {
                let exprs = self.fold_exprs(exprs)?;

                // flatten
                let exprs = exprs
                    .into_iter()
                    .flat_map(|e| match e.kind {
                        ExprKind::Tuple(items) if e.flatten => items,
                        _ => vec![e],
                    })
                    .collect_vec();

                Expr {
                    kind: ExprKind::Tuple(exprs),
                    ..node
                }
            }

            item => Expr {
                kind: fold_expr_kind(self, item)?,
                ..node
            },
        };
        self.finish_expr_resolve(r, id, *alias, *span)
    }
}

impl super::Resolver<'_> {
    fn finish_expr_resolve(
        &mut self,
        expr: Expr,
        id: usize,
        alias: Option<String>,
        span: Option<Span>,
    ) -> Result<Expr> {
        let mut r = Box::new(self.maybe_static_eval(expr)?);

        r.id = r.id.or(Some(id));
        r.alias = r.alias.or(alias);
        r.span = r.span.or(span);

        if r.ty.is_none() {
            r.ty = self.infer_type(&r)?;
        }
        if let Some(ty) = &mut r.ty {
            if ty.is_relation() {
                if let Some(alias) = r.alias.take() {
                    // This is relation wrapping operation.
                    // Convert:
                    //     alias = r
                    // into:
                    //     _local.select {alias = _local.this} r

                    let expr = Expr::new(ExprKind::FuncCall(FuncCall {
                        name: Box::new(Expr::new(ExprKind::Ident(Ident::from_path(vec![
                            NS_LOCAL, "select",
                        ])))),
                        args: vec![
                            Expr::new(ExprKind::Tuple(vec![Expr {
                                alias: Some(alias),
                                ..Expr::new(Ident::from_path(vec![NS_LOCAL, NS_THIS]))
                            }])),
                            *r,
                        ],
                        named_args: Default::default(),
                    }));
                    return self.fold_expr(expr);
                }
            }
        }

        Ok(*r)
    }

    pub fn resolve_column_exclusion(&mut self, expr: Expr) -> Result<Expr> {
        let expr = self.fold_expr(expr)?;
        let except = self.coerce_into_tuple(expr)?;

        self.fold_expr(Expr::new(ExprKind::All {
            within: Box::new(Expr::new(Ident::from_path(vec![NS_LOCAL, NS_THIS]))),
            except: Box::new(except),
        }))
    }

    pub fn resolve_indirection(
        &mut self,
        base: &Ty,
        indirection: &IndirectionKind,
        pos_offset: usize,
    ) -> Result<(usize, Option<Ty>)> {
        // special case: generic type param inference
        if let TyKind::Ident(fq_ident) = &base.kind {
            // base should be resolved, so idents can only be fully-qualified references
            // to generic type parameters
            let generic_decl = self.root_mod.module.get(fq_ident).unwrap();
            let candidate_ty = generic_decl.kind.as_generic_param().unwrap();

            // if candidate is a tuple
            if let Some((candidate_ty, _)) = candidate_ty {
                let candidate_ty = candidate_ty.clone();

                // try to resolve indirection in the existing candidate
                let res = self.resolve_indirection(&candidate_ty, indirection, pos_offset);
                if res.is_ok() {
                    return res;
                } else {
                    // fallback to inferring a new tuple field
                    if candidate_ty.kind.is_tuple() {
                        return Ok(self.infer_tuple_field_of_generic(
                            fq_ident,
                            indirection,
                            pos_offset,
                        ));
                    }
                }
            } else {
                return Ok(self.infer_tuple_field_of_generic(fq_ident, indirection, pos_offset));
            }
        }

        let TyKind::Tuple(fields) = &base.kind else {
            let mut e = Error::new_simple(format!(
                "cannot lookup fields in {} type",
                base.kind.as_ref().to_lowercase()
            ));
            if let TyKind::Ident(fq_ident) = &base.kind {
                let generic_decl = self.root_mod.module.get(fq_ident).unwrap();
                let candidate_ty = generic_decl.kind.as_generic_param().unwrap();
                if let Some((candidate_ty, _)) = candidate_ty {
                    e = e.push_hint(format!("generic={}", write_ty(candidate_ty)))
                }
            }

            return Err(e);
        };

        match indirection {
            IndirectionKind::Name(field_name) => {
                // lookup in Single fields
                let mut res = fields
                    .iter()
                    .enumerate()
                    .find_map(|(pos, field)| match field {
                        TyTupleField::Single(Some(n), f_ty) if n == field_name => {
                            Some((pos, f_ty.clone()))
                        }
                        _ => None,
                    });

                // fallback: look into Unpacks
                if res.is_none() {
                    for (pos, ty_field) in fields.iter().enumerate() {
                        let TyTupleField::Unpack(Some(unpack_ty)) = ty_field else {
                            continue;
                        };
                        match self.resolve_indirection(unpack_ty, indirection, pos) {
                            Ok(r) => {
                                res = Some(r);
                                break;
                            }
                            Err(e) => {
                                log::debug!("cannot lookup into Unpack: {e:?}");
                                continue;
                            }
                        }
                    }
                }

                res.ok_or_else(|| {
                    Error::new_simple(format!(
                        "cannot lookup field `{field_name}` in tuple {}",
                        write_ty(base)
                    ))
                })
            }
            IndirectionKind::Position(position) => {
                let pos = *position as usize + pos_offset;

                let Some(field) = fields.get(pos) else {
                    return Err(Error::new_simple(format!(
                        "cannot lookup field `{position}` in tuple {}, which only has {} fields",
                        write_ty(base),
                        fields.len(),
                    )));
                };
                let ty = field.as_single().unwrap().1.clone();
                Ok((pos, ty))
            }
        }
    }
}
