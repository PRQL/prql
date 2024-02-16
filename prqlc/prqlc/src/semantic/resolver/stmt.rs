use crate::ast::{Ty, TyTupleField};
use crate::ir::decl::{DeclKind, TableDecl, TableExpr};
use crate::ir::pl::*;
use crate::Result;

impl super::Resolver<'_> {
    /// Entry point to the resolver.
    /// fq_ident must point to an unresolved declaration.
    pub fn resolve_decl(&mut self, fq_ident: Ident) -> Result<()> {
        // take decl out of the module
        let mut decl = {
            let module = self.root_mod.module.get_submodule_mut(&fq_ident.path);
            module.unwrap().names.remove(&fq_ident.name).unwrap()
        };
        let stmt = decl.kind.into_unresolved().unwrap();

        // resolve
        match stmt {
            StmtKind::QueryDef(d) => {
                decl.kind = DeclKind::QueryDef(*d);
            }
            StmtKind::ModuleDef(_) => {
                unreachable!("module def cannot be unresolved at this point")
                // it should have been converted into Module in resolve_decls::init_module_tree
            }
            StmtKind::VarDef(var_def) => {
                let mut def = self.fold_var_def(var_def)?;

                if let Some(ExprKind::Func(closure)) = def.value.as_mut().map(|x| &mut x.kind) {
                    if closure.name_hint.is_none() {
                        closure.name_hint = Some(fq_ident.clone());
                    }
                }

                let expected_ty = fold_type_opt(self, def.ty)?;

                decl.kind = match def.value {
                    Some(mut def_value) => {
                        // var value is provided

                        // validate type
                        if expected_ty.is_some() {
                            let who = || Some(fq_ident.name.clone());
                            self.validate_expr_type(&mut def_value, expected_ty.as_ref(), &who)?;
                        }

                        prepare_expr_decl(def_value)
                    }
                    None => {
                        // var value is not provided

                        // is this a relation?
                        if expected_ty.as_ref().map_or(false, |t| t.is_relation()) {
                            // treat this var as a TableDecl
                            DeclKind::TableDecl(TableDecl {
                                ty: expected_ty,
                                expr: TableExpr::LocalTable,
                            })
                        } else {
                            // treat this var as a param
                            let mut expr =
                                Box::new(Expr::new(ExprKind::Param(fq_ident.name.clone())));
                            expr.ty = expected_ty;
                            DeclKind::Expr(expr)
                        }
                    }
                };
            }
            StmtKind::TypeDef(ty_def) => {
                let value = if let Some(value) = ty_def.value {
                    value
                } else {
                    Ty::new(Literal::Null)
                };

                let ty = fold_type_opt(self, Some(value))?.unwrap();
                let mut ty = super::types::normalize_type(ty);
                ty.name = Some(fq_ident.name.clone());

                decl.kind = DeclKind::Ty(ty);
            }
            StmtKind::ImportDef(target) => {
                decl.kind = DeclKind::Import(target.name);
            }
        };

        // put decl back in
        {
            let module = self.root_mod.module.get_submodule_mut(&fq_ident.path);
            module.unwrap().names.insert(fq_ident.name, decl);
        }
        Ok(())
    }
}

fn prepare_expr_decl(value: Box<Expr>) -> DeclKind {
    match &value.lineage {
        Some(frame) => {
            let columns = (frame.columns.iter())
                .map(|col| match col {
                    LineageColumn::All { .. } => TyTupleField::Wildcard(None),
                    LineageColumn::Single { name, .. } => {
                        TyTupleField::Single(name.as_ref().map(|n| n.name.clone()), None)
                    }
                })
                .collect();
            let ty = Some(Ty::relation(columns));

            let expr = TableExpr::RelationVar(value);
            DeclKind::TableDecl(TableDecl { ty, expr })
        }
        _ => DeclKind::Expr(value),
    }
}
