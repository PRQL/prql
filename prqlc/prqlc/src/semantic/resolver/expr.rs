use crate::codegen::write_ty;
use crate::Result;
use itertools::Itertools;

use crate::ast::{Ty, TyKind, TyTupleField};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::semantic::resolver::{flatten, Resolver};
use crate::semantic::{NS_LOCAL, NS_SELF, NS_STD, NS_THAT, NS_THIS};
use crate::{Error, Reason, Span, WithErrorInfo};

impl PlFold for Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_type(&mut self, ty: Ty) -> Result<Ty> {
        Ok(match ty.kind {
            TyKind::Ident(ident) => {
                self.root_mod.local_mut().shadow(NS_THIS);
                self.root_mod.local_mut().shadow(NS_THAT);

                let fq_ident = self.resolve_ident(&ident)?;

                let decl = self.root_mod.module.get(&fq_ident).unwrap();
                let ty = match &decl.kind {
                    DeclKind::Ty(ty) => {
                        // materialize into the referred type
                        let mut ty = ty.clone();
                        ty.name = ty.name.or(Some(fq_ident.name));
                        ty
                    }

                    DeclKind::GenericParam(_) => {
                        // leave as an ident
                        Ty {
                            kind: TyKind::Ident(fq_ident),
                            ..ty
                        }
                    }

                    DeclKind::Unresolved(_) => {
                        return Err(Error::new_assert(format!(
                            "bad resolution order: unresolved {fq_ident} while resolving {}",
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

                self.root_mod.local_mut().unshadow(NS_THIS);
                self.root_mod.local_mut().unshadow(NS_THAT);

                ty
            }
            TyKind::Tuple(fields) => {
                let mut new_fields = Vec::new();
                for field in fields {
                    match field {
                        TyTupleField::Single(name, Some(ty)) => {
                            // standard folding
                            let ty = self.fold_type(ty)?;
                            new_fields.push(TyTupleField::Single(name, Some(ty)));
                        }
                        TyTupleField::Unpack(Some(ty)) => {
                            let ty = self.fold_type(ty)?;

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
                Ty {
                    kind: TyKind::Tuple(new_fields),
                    ..ty
                }
            }
            _ => fold_type(self, ty)?,
        })
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        let value = match var_def.value {
            Some(value) if matches!(value.kind, ExprKind::Func(_)) => Some(value),
            Some(value) => Some(Box::new(flatten::Flattener::fold(self.fold_expr(*value)?))),
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
                            let closure = self.fold_function_types(closure.clone())?;

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
                let TyKind::Tuple(fields) = &ty.kind else {
                    return Err(Error::new_simple(format!(
                        "cannot lookup fields in type {}",
                        write_ty(ty)
                    ))
                    .with_span(*span));
                };

                let (position, ty_field) = match field {
                    IndirectionKind::Name(field_name) => {
                        let mut field = fields.iter().find_position(|f| match f {
                            TyTupleField::Single(Some(n), _) => n == &field_name,
                            _ => false,
                        });

                        // fallback: infer a field of a generic type argument
                        if field.is_none() {
                            if let Some((pos_of_unpack, generic_ident)) = fields
                                .iter()
                                .enumerate()
                                .find_map(|(pos, x)| as_field_unpack_of_ident(x).map(|i| (pos, i)))
                            {
                                let (pos, ty_field) =
                                    self.infer_tuple_field_of_generic(generic_ident, &field_name)?;
                                field = Some((pos + pos_of_unpack, ty_field));
                            }
                        }

                        let Some((position, field)) = field else {
                            return Err(Error::new_simple(format!(
                                "cannot lookup field `{field_name}` in tuple {}",
                                write_ty(ty)
                            ))
                            .with_span(*span));
                        };
                        (position as i64, field)
                    }
                    IndirectionKind::Position(position) => {
                        let Some(field) = fields.get(position as usize) else {
                            return Err(Error::new_simple(format!(
                                "cannot lookup field `{position}` in tuple {}, which only has {} fields",
                                write_ty(ty),
                                fields.len(),
                            )).with_span(*span));
                        };
                        (position, field)
                    }
                };
                Expr {
                    ty: ty_field.as_single().unwrap().1.clone(),
                    kind: ExprKind::Indirection {
                        base: Box::new(base),
                        field: IndirectionKind::Position(position),
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
                self.fold_function(func, *span)?
            }

            ExprKind::Func(closure) => self.fold_function(closure, *span)?,

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

fn as_field_unpack_of_ident(x: &TyTupleField) -> Option<&Ident> {
    x.as_unpack()
        .and_then(|x| x.as_ref())
        .and_then(|x| x.kind.as_ident())
}

impl Resolver<'_> {
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
}
