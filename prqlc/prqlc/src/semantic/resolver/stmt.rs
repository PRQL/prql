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

        // A module def for a name that's already a module merges into it rather
        // than replacing it. Inserting a fresh module used to discard whatever
        // was there, so the declarations of every earlier `module m` block for
        // the same `m` vanished without a word — including the ones a sibling
        // source file contributed to that module path.
        //
        // Merging is also what makes `module std` in std.prql work at all: the
        // name is already occupied by the placeholder `Module::new_root` puts
        // there. Names that clash *within* the merged module are still caught,
        // by the `declare` of each inner statement.
        match self.root_mod.module.get_mut(&ident) {
            Some(existing) if existing.kind.is_module() => {
                existing.annotations.extend(stmt.annotations);
            }
            // A non-module declaration under the same name is still replaced,
            // as std.prql relies on it: `type text` (a type primitive) and
            // `module text` (the string functions) both declare `std.text`.
            // See the follow-up issue linked from the PR.
            _ => {
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
                self.root_mod
                    .module
                    .insert(ident.clone(), decl)
                    .with_span(stmt.span)?;
            }
        }

        self.current_module_path.push(ident.name);
        self.fold_statements(module_def.stmts)?;
        self.current_module_path.pop();
        Ok(())
    }

    fn fold_import_def_stmt(&mut self, stmt: Stmt, ident: Ident) -> Result<()> {
        let target = stmt.kind.into_import_def().unwrap();
        let decl = DeclKind::Import(target.name);

        self.root_mod
            .declare(ident, decl, stmt.id, stmt.annotations)
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
    use std::path::PathBuf;

    use insta::assert_snapshot;

    use crate::tests::compile;

    /// Compile a multi-file project, the way `prqlc compile <dir>` does.
    fn compile_tree(files: &[(&str, &str)]) -> Result<String, crate::ErrorMessages> {
        anstream::ColorChoice::Never.write_global();
        let sources = crate::SourceTree::new(
            files
                .iter()
                .map(|(path, content)| (PathBuf::from(path), content.to_string())),
            None,
        );
        let pl = crate::prql_to_pl_tree(&sources)?;
        let rq = crate::pl_to_rq_tree(pl, &[], &[crate::semantic::NS_DEFAULT_DB.to_string()])?;
        crate::rq_to_sql(rq, &crate::Options::default().no_signature())
            .map_err(|e| e.composed(&sources))
    }

    #[test]
    fn module_def_merges_with_earlier_block() {
        // A second `module m` block used to replace the first outright, so `m.a`
        // silently disappeared.
        assert_snapshot!(compile(r"
        module m {
          let a = 1
        }
        module m {
          let b = 2
        }
        from t
        derive {x = m.a, y = m.b}
        ").unwrap(), @r"
        SELECT
          *,
          1 AS x,
          2 AS y
        FROM
          t
        ");
    }

    #[test]
    fn module_def_merges_across_files() {
        // `a.prql` contributes to module `a`, and so does a `module a` block in
        // the root file. Both sets of declarations have to survive.
        assert_snapshot!(compile_tree(&[
            ("a.prql", "let y = 2\n"),
            (
                "Main.prql",
                "module a {\n  let x = 1\n}\nfrom t\nderive {p = a.x, q = a.y}\n",
            ),
        ]).unwrap(), @r"
        SELECT
          *,
          1 AS p,
          2 AS q
        FROM
          t
        ");
    }

    #[test]
    fn duplicate_name_in_merged_module_is_an_error() {
        // Merging must not paper over a genuine clash between the two blocks.
        assert_snapshot!(compile(r"
        module m {
          let a = 1
        }
        module m {
          let a = 2
        }
        from t
        ").unwrap_err(), @"
        Error:
           ╭─[ :5:19 ]
           │
         5 │ ╭─▶         module m {
         6 │ ├─▶           let a = 2
           │ │
           │ ╰───────────────────────── duplicate declarations of m.a
        ───╯
        ");
    }

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
        ───╯
        ");
    }
}
