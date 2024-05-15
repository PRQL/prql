use itertools::Itertools;

use crate::ast::{Ty, TyKind};
use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::semantic::resolver::scope::LookupResult;
use crate::semantic::{NS_LOCAL, NS_STD, NS_THIS};
use crate::{Error, Result, Span, WithErrorInfo};

use super::tuple::StepOwned;

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

                let steps = self.resolve_indirection(ty, &field).with_span(*span)?;

                let expr = self.apply_indirections(base, steps);
                Expr {
                    id: expr.id,
                    kind: expr.kind,
                    ty: expr.ty,
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
    pub fn resolve_indirection(
        &mut self,
        base: &Ty,
        indirection: &IndirectionKind,
    ) -> Result<Vec<StepOwned>> {
        match indirection {
            IndirectionKind::Name(name) => self.lookup_name_in_tuple(base, name),
            IndirectionKind::Position(pos) => {
                let step = super::tuple::lookup_position_in_tuple(base, *pos as usize)?
                    .ok_or_else(|| Error::new_simple("Out of bounds"))?;

                Ok(vec![step])
            }
        }
    }
}
