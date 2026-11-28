use itertools::Itertools;

use crate::ir::decl::{DeclKind, Module};
use crate::ir::pl;
use crate::ir::pl::PlFold;
use crate::pr::{Ty, TyKind, TyTupleField};
use crate::semantic::resolver::{flatten, types, Resolver};
use crate::semantic::{write_pl, NS_INFER, NS_SELF, NS_THAT, NS_THIS, STD_LIB_SOURCE_ID};
use crate::utils::IdGenerator;
use crate::Result;
use crate::{Error, Reason, Span, WithErrorInfo};

impl Resolver<'_> {
    /// Resolves the name in a type annotation to the type it declares.
    fn resolve_type_ident(&mut self, ident: &pl::Ident) -> Result<Ty> {
        let fq_ident = self.resolve_ident(ident)?;

        let decl = self.root_mod.module.get(&fq_ident).unwrap();
        let decl_ty = decl.kind.as_ty().ok_or_else(|| {
            Error::new(Reason::Expected {
                who: None,
                expected: "a type".to_string(),
                found: decl.to_string(),
            })
        })?;
        let mut ty = decl_ty.clone();
        ty.name = ty.name.or(Some(fq_ident.name));
        Ok(ty)
    }
}

impl pl::PlFold for Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<pl::Stmt>) -> Result<Vec<pl::Stmt>> {
        unreachable!()
    }

    fn fold_type(&mut self, ty: Ty) -> Result<Ty> {
        Ok(match ty.kind {
            TyKind::Ident(ident) => {
                self.root_mod.module.shadow(NS_THIS);
                self.root_mod.module.shadow(NS_THAT);

                // Resolve and look up the name while `this` and `that` are
                // shadowed, then restore them whether or not that succeeded.
                let resolved = self.resolve_type_ident(&ident);

                self.root_mod.module.unshadow(NS_THIS);
                self.root_mod.module.unshadow(NS_THAT);

                resolved.map_err(|e| e.with_span_fallback(ty.span))?
            }
            _ => pl::fold_type(self, ty)?,
        })
    }

    fn fold_var_def(&mut self, var_def: pl::VarDef) -> Result<pl::VarDef> {
        let value = match var_def.value {
            Some(value) if matches!(value.kind, pl::ExprKind::Func(_)) => Some(value),
            Some(value) => Some(Box::new(flatten::Flattener::fold(self.fold_expr(*value)?))),
            None => None,
        };

        Ok(pl::VarDef {
            name: var_def.name,
            value,
            ty: var_def.ty.map(|x| self.fold_type(x)).transpose()?,
        })
    }

    fn fold_expr(&mut self, node: pl::Expr) -> Result<pl::Expr> {
        // Only the expression folded right after the flag is set is the step.
        let is_pipeline_step = std::mem::take(&mut self.in_pipeline_step);

        if node.id.is_some() && !matches!(node.kind, pl::ExprKind::Func(_)) {
            return Ok(node);
        }

        let id = if node
            .span
            .is_some_and(|span| span.source_id == STD_LIB_SOURCE_ID)
        {
            self.id.gen_sys()
        } else {
            self.id.gen()
        };
        let alias = node.alias.clone();
        let span = node.span;

        if let Some(span) = span {
            self.root_mod.span_map.insert(id, span);
        }

        log::trace!("folding expr [{id:?}] {node:?}");

        let r = match node.kind {
            pl::ExprKind::Ident(ident) => {
                log::debug!("resolving ident {ident}...");
                log::trace!(
                    "... candidates: {:#?}",
                    self.root_mod
                        .module
                        .all_names(None)
                        .into_iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                );
                let fq_ident = self
                    .resolve_ident(&ident)
                    .map_err(|e| e.with_span(node.span))?;
                log::debug!("... resolved to {fq_ident}");
                let entry = self.root_mod.module.get(&fq_ident).unwrap();
                log::debug!("... which is {entry}");

                match &entry.kind {
                    DeclKind::Infer(_) => pl::Expr {
                        kind: pl::ExprKind::Ident(fq_ident),
                        target_id: entry.declared_at,
                        ..node
                    },
                    DeclKind::Column(target_id) => pl::Expr {
                        kind: pl::ExprKind::Ident(fq_ident),
                        target_id: Some(*target_id),
                        ..node
                    },

                    DeclKind::TableDecl(_) => {
                        let input_name = ident.name.clone();

                        let lineage = self.lineage_of_table_decl(&fq_ident, input_name, id);

                        pl::Expr {
                            kind: pl::ExprKind::Ident(fq_ident),
                            ty: Some(ty_of_lineage(&lineage)),
                            lineage: Some(lineage),
                            alias: None,
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => match &expr.kind {
                        pl::ExprKind::Func(closure) => {
                            let closure = self.fold_function_types(closure.clone())?;

                            let expr = pl::Expr::new(pl::ExprKind::Func(closure));

                            if self.in_func_call_name {
                                expr
                            } else {
                                self.fold_expr(expr)?
                            }
                        }
                        _ => {
                            // An alias at the reference, as in `select {z = x}`,
                            // takes precedence over one in the definition, as
                            // in `let x = (y = 5)`.
                            let mut expr = expr.as_ref().clone();
                            expr.alias = alias.clone().or(expr.alias);
                            self.fold_expr(expr)?
                        }
                    },

                    DeclKind::InstanceOf(_, ty) => {
                        let ty = ty.clone();

                        let fields = self.construct_wildcard_include(&fq_ident);

                        pl::Expr {
                            kind: pl::ExprKind::Tuple(fields),
                            ty,
                            ..node
                        }
                    }

                    DeclKind::Ty(_) => {
                        return Err(Error::new(Reason::Expected {
                            who: None,
                            expected: "a value".to_string(),
                            found: "a type".to_string(),
                        })
                        .with_span(span));
                    }

                    _ => pl::Expr {
                        kind: pl::ExprKind::Ident(fq_ident),
                        ..node
                    },
                }
            }

            // special case: handle the syntax !{tuple..} via resolve_column_exclusion
            pl::ExprKind::FuncCall(pl::FuncCall {
                name,
                args,
                named_args,
            }) if (name.kind.as_ident()).is_some_and(|i| i.to_string() == "std.not")
                && args.len() == 1
                && named_args.is_empty()
                && matches!(args[0].kind, pl::ExprKind::Tuple(_)) =>
            {
                let arg = args.into_iter().exactly_one().unwrap();
                self.resolve_column_exclusion(arg)?
            }

            pl::ExprKind::FuncCall(pl::FuncCall {
                name,
                args,
                named_args,
            }) => {
                // fold function name
                self.default_namespace = None;
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                // A pipeline step is a call whose name is itself a call
                // (`select x` in `from t | select x`); flag the inner call so
                // it can explain its own errors.
                self.in_pipeline_step = matches!(name.kind, pl::ExprKind::FuncCall(_));
                let name = self.fold_expr(*name);
                self.in_pipeline_step = false;
                self.in_func_call_name = old;
                let name = Box::new(name?);

                let func = name.try_cast(|n| n.into_func(), None, "a function")?;

                // Keep the args only when they already fill every positional
                // param, including the relation the pipeline supplies again.
                // Such a step can't compile, so valid steps skip the copy.
                let overfilled = (is_pipeline_step && step_overfills(&func, args.len()))
                    .then(|| (func.as_debug_name().to_string(), args.clone()));

                // fold function
                let folded = self
                    .apply_args_to_closure(func, args, named_args)
                    .with_span_fallback(span)
                    .and_then(|func| self.fold_function(func, span));
                match (folded, overfilled) {
                    (Ok(expr), _) => expr,
                    (Err(_), Some((debug_name, args))) => {
                        return Err(self.explain_overfilled_step(&debug_name, &args, span))
                    }
                    (Err(err), None) => return Err(err),
                }
            }

            pl::ExprKind::Func(closure) => self.fold_function(closure, span)?,

            pl::ExprKind::Tuple(exprs) => {
                let exprs = self.fold_exprs(exprs)?;

                // flatten
                let exprs = exprs
                    .into_iter()
                    .flat_map(|e| match e.kind {
                        pl::ExprKind::Tuple(items) if e.flatten => items,
                        _ => vec![e],
                    })
                    .collect_vec();

                pl::Expr {
                    kind: pl::ExprKind::Tuple(exprs),
                    ..node
                }
            }

            item => pl::Expr {
                kind: pl::fold_expr_kind(self, item)?,
                ..node
            },
        };
        self.finish_expr_resolve(r, id, alias, span)
    }
}

impl Resolver<'_> {
    /// Explains why a pipeline step whose arguments already fill every
    /// positional param of a transform, including the relation, failed: the
    /// pipeline supplies the relation once more. In `select x = sum salary`,
    /// the args are `x = sum` and `salary`, so `salary` is resolved as the
    /// relation and would fail with an unhelpful "Unknown name".
    fn explain_overfilled_step(
        &mut self,
        debug_name: &str,
        args: &[pl::Expr],
        span: Option<Span>,
    ) -> Error {
        let mut error = Error::new_simple(format!("Too many arguments to function `{debug_name}`"))
            .with_span(span);

        // `x = sum salary` passes `x = sum` and `salary` as two arguments
        let split_call = (args.iter().tuple_windows())
            .find(|(arg, _)| arg.kind.is_ident() && self.resolves_to_function(arg));
        if let Some((func_arg, next)) = split_call {
            let callee = write_pl(pl::Expr {
                alias: None,
                ..func_arg.clone()
            });
            let next = write_pl(next.clone());
            let (written, suggested) = match &func_arg.alias {
                Some(alias) => (
                    format!("{alias} = {callee}"),
                    format!("{alias} = ({callee} {next})"),
                ),
                None => (callee.clone(), format!("({callee} {next})")),
            };
            error = error.push_hint(format!(
                "`{written}` and `{next}` are separate arguments; parentheses might be missing: `{suggested}`"
            ));
        } else {
            error = error
                .push_hint("the pipeline supplies the relation, so an argument might be extra");
        }
        error
    }

    /// Whether an unresolved expression names a function. Only used while
    /// building an error, so any declarations inferred meanwhile don't matter.
    fn resolves_to_function(&mut self, expr: &pl::Expr) -> bool {
        let old = self.in_func_call_name;
        self.in_func_call_name = true;
        let resolved = self.fold_expr(pl::Expr {
            alias: None,
            ..expr.clone()
        });
        self.in_func_call_name = old;
        resolved.is_ok_and(|e| e.kind.is_func())
    }

    fn finish_expr_resolve(
        &mut self,
        expr: pl::Expr,
        id: usize,
        alias: Option<String>,
        span: Option<Span>,
    ) -> Result<pl::Expr> {
        let mut r = Box::new(self.maybe_static_eval(expr)?);

        r.id = r.id.or(Some(id));
        r.alias = r.alias.or(alias);
        r.span = r.span.or(span);

        if r.ty.is_none() {
            r.ty = Resolver::infer_type(&r)?;
        }
        if r.lineage.is_none() {
            if let pl::ExprKind::TransformCall(call) = &r.kind {
                r.lineage = Some(call.infer_lineage()?);
            } else if let Some(relation_columns) = r.ty.as_ref().and_then(|t| t.as_relation()) {
                log::debug!("found a relational type without lineage: declaring a new table for it: {relation_columns:?}");

                // lineage from ty
                let columns = Some(relation_columns.clone());

                let name = r.alias.clone();
                let frame = self.declare_table_for_literal(id, columns, name);

                r.lineage = Some(frame);
            }
        }
        if let Some(lineage) = &mut r.lineage {
            if let Some(alias) = r.alias.take() {
                lineage.rename(alias.clone());

                if let Some(ty) = &mut r.ty {
                    types::rename_relation(&mut ty.kind, alias);
                }
            }
        }
        Ok(*r)
    }

    pub fn resolve_column_exclusion(&mut self, expr: pl::Expr) -> Result<pl::Expr> {
        let expr = self.fold_expr(expr)?;
        let except = self.coerce_into_tuple(expr)?;

        self.fold_expr(pl::Expr::new(pl::ExprKind::All {
            within: Box::new(pl::Expr::new(pl::Ident::from_name(NS_THIS))),
            except: Box::new(except),
        }))
    }

    pub fn construct_wildcard_include(&mut self, module_fq_self: &pl::Ident) -> Vec<pl::Expr> {
        let module_fq = module_fq_self.clone().pop().unwrap();

        let decl = self.root_mod.module.get(&module_fq).unwrap();
        let module = decl.kind.as_module().unwrap();

        let prefix = module_fq.iter().collect_vec();
        Self::construct_tuple_from_module(&mut self.id, &prefix, module)
    }

    pub fn construct_tuple_from_module(
        id: &mut IdGenerator<usize>,
        prefix: &[&String],
        module: &Module,
    ) -> Vec<pl::Expr> {
        let mut res = Vec::new();

        if let Some(decl) = module.names.get(NS_INFER) {
            let wildcard_field = pl::Expr {
                id: Some(id.gen()),
                target_id: decl.declared_at,
                flatten: true,
                ty: Some(Ty::new(TyKind::Tuple(vec![TyTupleField::Wildcard(None)]))),
                ..pl::Expr::new(pl::Ident::from_name(NS_SELF))
            };
            return vec![wildcard_field];
        }

        for (name, decl) in module.names.iter().sorted_by_key(|(_, d)| d.order) {
            res.push(match &decl.kind {
                DeclKind::Module(submodule) => {
                    let prefix = [prefix.to_vec(), vec![name]].concat();
                    let sub_fields = Self::construct_tuple_from_module(id, &prefix, submodule);
                    pl::Expr {
                        id: Some(id.gen()),
                        alias: Some(name.clone()),
                        ..pl::Expr::new(pl::ExprKind::Tuple(sub_fields))
                    }
                }
                DeclKind::Column(target_id) => pl::Expr {
                    id: Some(id.gen()),
                    target_id: Some(*target_id),
                    // alias: Some(name.clone()),
                    ..pl::Expr::new(pl::Ident::from_path([prefix.to_vec(), vec![name]].concat()))
                },
                _ => continue,
            });
        }
        res
    }

    pub fn construct_wildcard_from_lineage(
        &mut self,
        prefix: &[&String],
        expr: &pl::Expr,
    ) -> Vec<pl::Expr> {
        let Some(lineage) = &expr.lineage else {
            return vec![];
        };

        log::trace!("construct_wildcard_from_lineage: {lineage:#?}");

        lineage
            .columns
            .iter()
            .filter_map(|col| match col {
                pl::LineageColumn::All { input_id, .. } => Some(pl::Expr {
                    id: Some(self.id.gen()),
                    target_id: Some(*input_id),
                    flatten: true,
                    ty: Some(Ty::new(TyKind::Tuple(vec![TyTupleField::Wildcard(None)]))),
                    ..pl::Expr::new(pl::Ident::from_name(NS_SELF))
                }),
                pl::LineageColumn::Single {
                    target_id,
                    target_name: Some(target_name),
                    ..
                } => Some(pl::Expr {
                    id: Some(self.id.gen()),
                    target_id: Some(*target_id),
                    alias: Some(target_name.to_string()),
                    ..pl::Expr::new(pl::Ident::from_path(
                        [prefix.to_vec(), vec![target_name]].concat(),
                    ))
                }),
                pl::LineageColumn::Single {
                    target_id,
                    name: Some(ident),
                    ..
                } => Some(pl::Expr {
                    id: Some(self.id.gen()),
                    target_id: Some(*target_id),
                    alias: Some(ident.name.to_string()),
                    ..pl::Expr::new(ident.clone())
                }),
                _ => None,
            })
            .collect()
    }
}

fn ty_of_lineage(lineage: &pl::Lineage) -> Ty {
    Ty::relation(
        lineage
            .columns
            .iter()
            .map(|col| match col {
                pl::LineageColumn::All { .. } => TyTupleField::Wildcard(None),
                pl::LineageColumn::Single { name, .. } => {
                    TyTupleField::Single(name.as_ref().map(|i| i.name.clone()), None)
                }
            })
            .collect(),
    )
}

/// Whether a pipeline step's explicit args fill every positional param of a
/// function whose last param is a relation, leaving none for the pipeline.
fn step_overfills(func: &pl::Func, n_args: usize) -> bool {
    let takes_relation_last = (func.params.last())
        .and_then(|p| p.ty.as_ref())
        .is_some_and(|t| t.is_relation());
    takes_relation_last && n_args >= func.params.len().saturating_sub(func.args.len())
}
