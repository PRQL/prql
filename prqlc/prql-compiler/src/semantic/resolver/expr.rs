use anyhow::Result;
use itertools::Itertools;

use prqlc_ast::{TupleField, Ty, TyKind};

use crate::ir::decl::DeclKind;
use crate::ir::pl::*;
use crate::semantic::resolver::{flatten, transforms, types, Resolver};
use crate::semantic::{write_pl, NS_THAT, NS_THIS};
use crate::{Error, Reason, WithErrorInfo};

impl PlFold for Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_type(&mut self, ty: Ty) -> Result<Ty> {
        Ok(match ty.kind {
            TyKind::Ident(ident) => {
                self.root_mod.module.shadow(NS_THIS);
                self.root_mod.module.shadow(NS_THAT);

                let fq_ident = self.resolve_ident(&ident)?;

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

                self.root_mod.module.unshadow(NS_THIS);
                self.root_mod.module.unshadow(NS_THAT);

                ty
            }
            _ => fold_type(self, ty)?,
        })
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        let value = if matches!(var_def.value.kind, ExprKind::Func(_)) {
            var_def.value
        } else {
            Box::new(flatten::Flattener::fold(self.fold_expr(*var_def.value)?))
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
        let alias = node.alias.clone();
        let span = node.span;

        if let Some(span) = span {
            self.root_mod.span_map.insert(id, span);
        }

        log::trace!("folding expr {node:?}");

        let r = match node.kind {
            ExprKind::Ident(ident) => {
                log::debug!("resolving ident {ident}...");
                let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
                log::debug!("... resolved to {fq_ident}");
                let entry = self.root_mod.module.get(&fq_ident).unwrap();
                log::debug!("... which is {entry}");

                match &entry.kind {
                    DeclKind::Infer(_) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        target_id: entry.declared_at,
                        ..node
                    },
                    DeclKind::Column(target_id) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        target_id: Some(*target_id),
                        ..node
                    },

                    DeclKind::TableDecl(_) => {
                        let input_name = ident.name.clone();

                        let lineage = self.lineage_of_table_decl(&fq_ident, input_name, id);

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(ty_of_lineage(&lineage)),
                            lineage: Some(lineage),
                            alias: None,
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => match &expr.kind {
                        ExprKind::Func(closure) => {
                            let closure = self.fold_function_types(*closure.clone())?;

                            let expr = Expr::new(ExprKind::Func(Box::new(closure)));

                            if self.in_func_call_name {
                                expr
                            } else {
                                self.fold_expr(expr)?
                            }
                        }
                        _ => self.fold_expr(expr.as_ref().clone())?,
                    },

                    DeclKind::InstanceOf(_) => {
                        return Err(Error::new_simple(
                            "table instance cannot be referenced directly",
                        )
                        .with_span(span)
                        .push_hint("did you forget to specify the column name?")
                        .into());
                    }

                    DeclKind::Ty(_) => {
                        return Err(Error::new(Reason::Expected {
                            who: None,
                            expected: "a value".to_string(),
                            found: "a type".to_string(),
                        })
                        .with_span(span)
                        .into());
                    }

                    _ => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ..node
                    },
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
                self.default_namespace = None;
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                let name = self.fold_expr(*name)?;
                self.in_func_call_name = old;

                let func = *name.try_cast(|n| n.into_func(), None, "a function")?;

                // fold function
                let func = self.apply_args_to_closure(func, args, named_args)?;
                self.fold_function(func, span)?
            }

            ExprKind::Func(closure) => self.fold_function(*closure, span)?,

            ExprKind::All { within, except } => {
                let decl = self.root_mod.module.get(&within);

                // lookup ids of matched inputs
                let target_ids = decl
                    .and_then(|d| d.kind.as_module())
                    .iter()
                    .flat_map(|module| module.as_decls())
                    .sorted_by_key(|(_, decl)| decl.order)
                    .flat_map(|(_, decl)| match &decl.kind {
                        DeclKind::Column(target_id) => Some(*target_id),
                        DeclKind::Infer(_) => decl.declared_at,
                        _ => None,
                    })
                    .unique()
                    .collect();

                let kind = ExprKind::All { within, except };
                Expr {
                    kind,
                    target_ids,
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

            ExprKind::Array(exprs) => {
                let mut exprs = self.fold_exprs(exprs)?;

                // validate that all elements have the same type
                let mut expected_ty: Option<&Ty> = None;
                for expr in &mut exprs {
                    if expr.ty.is_some() {
                        if expected_ty.is_some() {
                            let who = || Some("array".to_string());
                            self.validate_expr_type(expr, expected_ty, &who)?;
                        }
                        expected_ty = expr.ty.as_ref();
                    }
                }

                Expr {
                    kind: ExprKind::Array(exprs),
                    ..node
                }
            }

            item => Expr {
                kind: fold_expr_kind(self, item)?,
                ..node
            },
        };
        let mut r = self.static_eval(r)?;
        r.id = r.id.or(Some(id));
        r.alias = r.alias.or(alias);
        r.span = r.span.or(span);

        if r.ty.is_none() {
            r.ty = types::infer_type(&r)?;
        }
        if r.lineage.is_none() {
            if let ExprKind::TransformCall(call) = &r.kind {
                r.lineage = Some(call.infer_type(self.root_mod)?);
            } else if let Some(relation_columns) = r.ty.as_ref().and_then(|t| t.as_relation()) {
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
        Ok(r)
    }
}

impl Resolver<'_> {
    fn resolve_column_exclusion(&mut self, expr: Expr) -> Result<Expr> {
        let expr = self.fold_expr(expr)?;
        let tuple = transforms::coerce_into_tuple_and_flatten(expr)?;
        let except: Vec<Expr> = tuple
            .into_iter()
            .map(|e| match e.kind {
                ExprKind::Ident(_) | ExprKind::All { .. } => Ok(e),
                _ => Err(Error::new(Reason::Expected {
                    who: Some("exclusion".to_string()),
                    expected: "column name".to_string(),
                    found: format!("`{}`", write_pl(e)),
                })),
            })
            .try_collect()?;

        self.fold_expr(Expr::new(ExprKind::All {
            within: Ident::from_name(NS_THIS),
            except,
        }))
    }
}

fn ty_of_lineage(lineage: &Lineage) -> Ty {
    Ty::relation(
        lineage
            .columns
            .iter()
            .map(|col| match col {
                LineageColumn::All { .. } => TupleField::Wildcard(None),
                LineageColumn::Single { name, .. } => TupleField::Single(
                    name.as_ref().map(|i| i.name.clone()),
                    Some(Ty::new(Literal::Null)),
                ),
            })
            .collect(),
    )
}
