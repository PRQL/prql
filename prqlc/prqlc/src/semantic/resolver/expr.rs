use itertools::Itertools;

use crate::ast::{Ty, TyKind, TyTupleField};
use crate::codegen::write_ty;
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::semantic::resolver::scope::LookupResult;
use crate::semantic::{NS_LOCAL, NS_STD, NS_THIS};
use crate::{Error, Result, Span, WithErrorInfo};

impl PlFold for super::Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_type(&mut self, ty: Ty) -> Result<Ty> {
        self.fold_type_actual(ty)
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        Ok(VarDef {
            name: var_def.name,
            value: match var_def.value {
                Some(value) => Some(Box::new(self.fold_expr(*value)?)),
                None => None,
            },
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
                log::debug!("resolving ident {ident:?}...");

                let result = self.lookup_ident(&ident, false).with_span(node.span)?;

                if let LookupResult::Indirect {
                    real_name,
                    indirections,
                } = result
                {
                    let mut ident = ident;
                    ident.name = real_name;

                    let mut expr = Expr {
                        kind: ExprKind::Ident(ident),
                        ..node
                    };
                    for indirection in indirections {
                        expr = Expr::new(ExprKind::Indirection {
                            base: Box::new(expr),
                            field: indirection,
                        });
                    }
                    expr.flatten = node.flatten;
                    self.fold_expr(expr)?
                } else {
                    let decl = self.get_ident(&ident, false).unwrap();

                    let log_debug = !ident.starts_with_part(NS_STD);
                    if log_debug {
                        log::debug!("... resolved to {decl}");
                    }

                    match &decl.kind {
                        DeclKind::Variable(ty) => Expr {
                            kind: ExprKind::Ident(ident),
                            ty: ty.clone(),
                            ..node
                        },

                        DeclKind::TupleField => {
                            unimplemented!();
                            // indirections.push(IndirectionKind::Name(ident.name));
                            // Expr::new(ExprKind::Ident(Ident::from_path(ident.path)))
                        }

                        DeclKind::Expr(expr) => {
                            // keep as ident, but pull in the type
                            let ty = expr.ty.clone().unwrap();

                            // if the type contains generics, we need to instantiate those
                            // generics into current function scope
                            let ty = self.instantiate_type(ty, id);

                            Expr {
                                kind: ExprKind::Ident(ident),
                                ty: Some(ty),
                                ..node
                            }
                        }

                        DeclKind::Ty(_) => {
                            return Err(Error::new_simple("expected a value, but found a type")
                                .with_span(*span));
                        }

                        DeclKind::Infer(_) => unreachable!(),
                        DeclKind::Unresolved(_) => {
                            return Err(Error::new_assert(format!(
                                "bad resolution order: unresolved {ident} while resolving {}",
                                self.debug_current_decl
                            )));
                        }

                        _ => Expr {
                            kind: ExprKind::Ident(ident),
                            ..node
                        },
                    }
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
                let func = Box::new(self.fold_expr(*name)?);
                self.in_func_call_name = old;

                // convert to function application
                let fn_app = self.apply_args_to_function(func, args, named_args)?;
                self.resolve_func_application(fn_app, *span)?
            }

            ExprKind::Func(func) => {
                let func = self.resolve_func(func)?;
                Expr {
                    kind: ExprKind::Func(func),
                    ..node
                }
            }

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
        if r.ty.is_none() {
            let generic = self.init_new_global_generic("E");
            r.ty = Some(Ty::new(TyKind::Ident(generic)));
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
                            NS_STD, "select",
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

    /// Resolve tuple indirections.
    /// For example, `base.indirection` where `base` has a tuple type.
    ///
    /// Returns the position of the tuple field within the base tuple.
    ///
    /// The the base case, `pos_offset` is 0.
    /// When resolving [TyTupleField::Unpack], `pos_offset` is the position
    /// of the unpack in the top-level tuple type.
    pub fn resolve_indirection(
        &mut self,
        base: &Ty,
        indirection: &IndirectionKind,
        pos_offset: usize,
    ) -> Result<(usize, Option<Ty>)> {
        // get base fields
        let fields = match &base.kind {
            TyKind::Tuple(fields) => fields,

            // special case: generic type param inference
            // this happens when `base` has type, for example, `A`, which is a generic type param
            TyKind::Ident(fq_ident) => {
                // base is resolved, so idents can only be fully-qualified references
                // to generic type parameters
                let generic_decl = self.get_ident(fq_ident, true).unwrap();
                let candidate_ty = generic_decl.kind.as_generic_param().unwrap();

                return if let Some((candidate_ty, _)) = candidate_ty {
                    // when we do have a candidate type
                    let candidate_ty = candidate_ty.clone();

                    // ... try to resolve indirection in the existing candidate
                    let res = self.resolve_indirection(&candidate_ty, indirection, pos_offset);

                    if res.is_err() && candidate_ty.kind.is_tuple() {
                        // ... and fallback to inferring a new tuple field when existing indirection fails
                        Ok(self.infer_tuple_field_of_generic(fq_ident, indirection, pos_offset))
                    } else {
                        res
                    }
                } else {
                    // when there is no candidate, infer that it is a tuple
                    Ok(self.infer_tuple_field_of_generic(fq_ident, indirection, pos_offset))
                };
            }

            _ => {
                return Err(Error::new_simple(format!(
                    "cannot lookup fields in {} type",
                    base.kind.as_ref().to_lowercase()
                )));
            }
        };

        match indirection {
            IndirectionKind::Name(field_name) => {
                // go trough all of the fields
                fields
                    .iter()
                    .enumerate()
                    .find_map(|(pos, field)| match field {
                        // match single fields on their name
                        TyTupleField::Single(Some(n), f_ty) if n == field_name => {
                            Some((pos, f_ty.clone()))
                        }
                        // recurse into unpacks
                        TyTupleField::Unpack(Some(unpack_ty)) => {
                            self.resolve_indirection(unpack_ty, indirection, pos).ok()
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Error::new_simple(format!(
                            "there is no field `{field_name}` in tuple {}",
                            write_ty(base)
                        ))
                    })
            }

            IndirectionKind::Position(position) => {
                let pos = *position as usize;

                let Some(field) = fields.get(pos) else {
                    return Err(Error::new_simple(format!(
                        "cannot lookup field `{pos}` in tuple {}, which only has {} fields",
                        write_ty(base),
                        fields.len(),
                    )));
                };
                let ty = field.as_single().unwrap().1.clone();
                Ok((pos + pos_offset, ty))
            }
        }
    }
}
