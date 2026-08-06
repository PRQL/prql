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

            match stmt.kind {
                StmtKind::QueryDef(_) => self.fold_query_def_stmt(stmt, ident)?,
                StmtKind::VarDef(_) => self.fold_var_def_stmt(stmt, ident)?,
                StmtKind::TypeDef(_) => self.fold_type_def_stmt(stmt, ident)?,
                StmtKind::ModuleDef(_) => self.fold_module_def_stmt(stmt, ident)?,
                StmtKind::ImportDef(_) => self.fold_import_def_stmt(stmt, ident)?,
            };
        }
        Ok(())
    }

    fn fold_query_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let query_def = stmt.kind.into_query_def().unwrap();
        let decl = DeclKind::QueryDef(*query_def);
        self.root_mod
            .declare(ident, decl, stmt.id, Vec::new())
            .with_span(stmt.span)?;
        Ok(())
    }

    fn fold_type_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let type_def = stmt.kind.into_type_def().unwrap();
        let mut ty = self.fold_type(type_def.value)?;
        ty.name = Some(ident.name.clone());

        let decl = DeclKind::Ty(ty);

        self.root_mod
            .declare(ident, decl, stmt.id, stmt.annotations)
            .with_span(stmt.span)?;
        Ok(())
    }

    fn fold_module_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let module_def = stmt.kind.into_module_def().unwrap();
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
        Ok(())
    }

    fn fold_import_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let target = stmt.kind.into_import_def().unwrap();
        let decl = DeclKind::Import(target.name);

        // `declare` rather than a direct `Module::insert`, so that a name
        // already taken is reported as a duplicate instead of being overwritten,
        // matching `let` and `type`.
        self.root_mod
            .declare(ident, decl, stmt.id, stmt.annotations)
            .push_hint("to import both, alias one of them: `import alias = path`")
            .with_span(stmt.span)?;
        Ok(())
    }

    fn fold_var_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let var_def = stmt.kind.into_var_def().unwrap();
        let mut def = self.fold_var_def(var_def)?;

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
                if expected_ty.as_ref().is_some_and(|t| t.is_relation()) {
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

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use crate::tests::compile;

    #[test]
    fn duplicate_import_is_an_error() {
        // Import defs bypassed the duplicate check that `let` and `type` get, so
        // the second `b` used to silently win.
        assert_snapshot!(compile(r"
        import a.b
        import c.b
        from t
        ").unwrap_err(), @"
        Error:
           ╭─[ :2:19 ]
           │
         2 │ ╭─▶         import a.b
         3 │ ├─▶         import c.b
           │ │
           │ ╰──────────────────────── duplicate declarations of b
           │
           │     Help: to import both, alias one of them: `import alias = path`
        ───╯
        ");
    }

    #[test]
    fn aliased_import_avoids_duplicate() {
        // `import d = …` is the documented way to bring in two targets that
        // would otherwise land on the same name.
        assert_snapshot!(compile(r"
        import a.b
        import d = c.b
        from t
        ").unwrap(), @"
        SELECT
          *
        FROM
          t
        ");
    }
}
