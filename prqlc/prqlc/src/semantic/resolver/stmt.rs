use std::collections::HashMap;

use crate::ir::decl::{Decl, DeclKind, Module, TableDecl, TableExpr};
use crate::ir::pl::*;
use crate::pr::{Ty, TyKind, TyTupleField};
use crate::Result;
use crate::WithErrorInfo;

impl super::Resolver<'_> {
    // entry point to the resolver
    pub fn fold_statements(&mut self, stmts: Vec<Stmt>) -> Result<()> {
        for mut stmt in stmts {
            stmt.id = Some(self.id.gen());
            if let Some(span) = stmt.span {
                self.root_mod.span_map.insert(stmt.id.unwrap(), span);
            }

            let ident = Ident {
                path: self.current_module_path.clone(),
                name: stmt.name().to_string(),
            };

            let mut def = match stmt.kind {
                StmtKind::QueryDef(d) => {
                    let decl = DeclKind::QueryDef(*d);
                    self.root_mod
                        .declare(ident, decl, stmt.id, Vec::new())
                        .with_span(stmt.span)?;
                    continue;
                }
                StmtKind::VarDef(var_def) => self.fold_var_def(var_def)?,
                StmtKind::TypeDef(ty_def) => {
                    let mut ty = self.fold_type(ty_def.value)?;
                    ty.name = Some(ident.name.clone());

                    let decl = DeclKind::Ty(ty);

                    self.root_mod
                        .declare(ident, decl, stmt.id, stmt.annotations)
                        .with_span(stmt.span)?;
                    continue;
                }
                StmtKind::ModuleDef(module_def) => {
                    self.current_module_path.push(ident.name);

                    let decl = Decl {
                        declared_at: stmt.id,
                        kind: DeclKind::Module(Module {
                            names: HashMap::new(),
                            redirects: Vec::new(),
                            shadowed: None,
                        }),
                        annotations: stmt.annotations,
                        ..Default::default()
                    };
                    let ident = Ident::from_path(self.current_module_path.clone());
                    self.root_mod
                        .module
                        .insert(ident, decl)
                        .with_span(stmt.span)?;

                    self.fold_statements(module_def.stmts)?;
                    self.current_module_path.pop();
                    continue;
                }
                StmtKind::ImportDef(target) => {
                    let decl = Decl {
                        declared_at: stmt.id,
                        kind: DeclKind::Import(target.name),
                        annotations: stmt.annotations,
                        ..Default::default()
                    };

                    self.root_mod
                        .module
                        .insert(ident, decl)
                        .with_span(stmt.span)?;
                    continue;
                }
            };

            if def.name == "main" {
                def.ty = Some(Ty::new(TyKind::Ident(Ident::from_path(vec![
                    "std", "relation",
                ]))));
            }

            if let Some(ExprKind::Func(closure)) = def.value.as_mut().map(|x| &mut x.kind) {
                if closure.name_hint.is_none() {
                    closure.name_hint = Some(ident.clone());
                }
            }

            let expected_ty = fold_type_opt(self, def.ty)?;

            let decl = match def.value {
                Some(mut def_value) => {
                    // var value is provided

                    // validate type
                    if expected_ty.is_some() {
                        let who = || Some(def.name.clone());
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
                        let mut expr = Box::new(Expr::new(ExprKind::Param(def.name)));
                        expr.ty = expected_ty;
                        DeclKind::Expr(expr)
                    }
                }
            };
            self.root_mod
                .declare(ident, decl, stmt.id, stmt.annotations)
                .with_span(stmt.span)?;
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
