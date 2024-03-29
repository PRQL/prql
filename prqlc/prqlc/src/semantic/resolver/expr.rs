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
                let decl_ty = decl.kind.as_ty().ok_or_else(|| {
                    if decl.kind.is_unresolved() {
                        Error::new_assert(format!(
                            "bad resolution order: unresolved {fq_ident} while resolving {}",
                            self.debug_current_decl
                        ))
                    } else {
                        Error::new(Reason::Expected {
                            who: None,
                            expected: "a type".to_string(),
                            found: decl.to_string(),
                        })
                    }
                    .with_span(ty.span)
                })?;
                let mut ty = decl_ty.clone();
                ty.name = ty.name.or(Some(fq_ident.name));

                self.root_mod.local_mut().unshadow(NS_THIS);
                self.root_mod.local_mut().unshadow(NS_THAT);

                ty
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
                log::debug!("resolving ident {ident}...");
                let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
                let log_debug = !fq_ident.starts_with_part(NS_STD);
                if log_debug {
                    log::debug!("... resolved to {fq_ident}")
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

                match &entry.kind {
                    DeclKind::Infer(_) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ..node
                    },
                    DeclKind::TupleField(ty) => {
                        let base = Expr::new(fq_ident.pop().unwrap());
                        let field = IndirectionKind::Position(entry.order as i64 - 1);
                        let ty = ty.clone();

                        let base = Box::new(self.fold_expr(base)?);
                        Expr {
                            kind: ExprKind::Indirection { base, field },
                            ty,
                            ..node
                        }
                    }

                    DeclKind::TableDecl(_) => {
                        let ty = self.ty_of_table_decl(&fq_ident);

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(ty),
                            alias: None,
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

                    DeclKind::InstanceOf(_, ty) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ty: ty.clone(),
                        ..node
                    },

                    DeclKind::Ty(_) => {
                        return Err(Error::new(Reason::Expected {
                            who: None,
                            expected: "a value".to_string(),
                            found: "a type".to_string(),
                        })
                        .with_span(*span));
                    }

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

                // if let IndirectionKind::Star = field {
                //     if node.alias.is_some() {
                //         return Err(
                //             Error::new_simple("alias not allowed on wildcards references")
                //                 .with_span(*span),
                //         );
                //     }
                //     Expr {
                //         flatten: true,
                //         ..base
                //     }
                // } else {
                let (position, ty_field) = match field {
                    IndirectionKind::Name(field_name) => {
                        let field = fields.iter().find_position(|f| match f {
                            TyTupleField::Single(Some(n), _) => n == &field_name,
                            _ => false,
                        });

                        let Some((position, field)) = field else {
                            return Err(Error::new_simple(format!(
                                "cannot lookup field {field_name} in tuple {}",
                                write_ty(ty)
                            ))
                            .with_span(*span));
                        };
                        (position as i64, field)
                    }
                    IndirectionKind::Position(position) => {
                        let Some(field) = fields.get(position as usize) else {
                            return Err(Error::new_simple(format!(
                                "cannot lookup field {position} in tuple {}, which only has {} fields",
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
            r.ty = Resolver::infer_type(&r)?;
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
