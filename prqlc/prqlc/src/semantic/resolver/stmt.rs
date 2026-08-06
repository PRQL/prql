use std::collections::HashMap;

use crate::ir::decl::{Decl, DeclKind, Module, TableDecl, TableExpr};
use crate::ir::pl::*;
use crate::pr::{Ty, TyKind, TyTupleField};
use crate::semantic::NS_SELF;
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
        // `type _self = …` inside `module m { … }` makes `m` itself usable as a
        // type, so name it after the module — `_self` is an implementation
        // detail that shouldn't reach error messages.
        ty.name = Some(if ident.name == NS_SELF {
            ident.path.last().unwrap_or(&ident.name).clone()
        } else {
            ident.name.clone()
        });

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

    /// `std.text` and `std.date` name both a module of functions and the
    /// corresponding primitive type; the type half lives under `_self`.
    #[test]
    fn test_module_self_type() {
        assert_snapshot!(crate::tests::compile(
            r#"
            let f = func x <std.text> -> x
            from t
            derive {y = f "a"}
            "#
        ).unwrap(), @r"
        SELECT
          *,
          'a' AS y
        FROM
          t
        ");
    }

    /// A `_self` type is still checked, and the mismatch is reported under the
    /// module's own name rather than leaking `_self`.
    #[test]
    fn test_module_self_type_mismatch() {
        assert_snapshot!(crate::tests::compile(
            r#"
            let f = func x <std.date> -> x
            from t
            derive {y = f 1}
            "#
        ).unwrap_err(), @"
        Error:
           ╭─[ :4:27 ]
           │
         4 │             derive {y = f 1}
           │                           ┬
           │                           ╰── function f, param `x` expected type `date`, but found type `int`
           │
           │ Help: Type `date` expands to `date`
        ───╯
        ");
    }

    /// A column named after a `_self` module is still an ordinary column: in
    /// expression position `date` means the column, not the type.
    #[test]
    fn test_module_self_type_does_not_shadow_columns() {
        assert_snapshot!(crate::tests::compile(
            "from t | filter date > @2020-01-01 | select {text}"
        ).unwrap(), @"
        SELECT
          text AS text
        FROM
          t
        WHERE
          date > DATE '2020-01-01'
        ");
    }

    /// A `_self` type is not a relation, so `text.*` is no more a wildcard than
    /// `math.*` is.
    #[test]
    fn test_module_self_type_is_not_a_wildcard_target() {
        assert_snapshot!(crate::tests::compile(
            "from t | select text.*"
        ).unwrap_err(), @"
        Error:
           ╭─[ :1:17 ]
           │
         1 │ from t | select text.*
           │                 ───┬──
           │                    ╰──── Unknown relation text
        ───╯
        ");
    }

    /// The module keeps working as a namespace of functions.
    #[test]
    fn test_module_self_does_not_shadow_functions() {
        assert_snapshot!(crate::tests::compile(
            "prql target:sql.postgres
             from t | derive {a = (text.lower name), b = (date.to_text \"%Y\" d)}"
        ).unwrap(), @"
        SELECT
          *,
          LOWER(name) AS a,
          TO_CHAR(d, 'YYYY') AS b
        FROM
          t
        ");
    }
}
