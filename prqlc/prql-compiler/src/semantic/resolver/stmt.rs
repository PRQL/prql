use anyhow::Result;
use std::collections::HashMap;

use crate::ir::decl::{Decl, DeclKind, Module, TableDecl, TableExpr};
use crate::ir::pl::*;
use crate::{Error, WithErrorInfo};

use super::{Resolver, NS_STD};

impl Resolver<'_> {
    // entry point to the resolver
    pub fn fold_statements(&mut self, stmts: Vec<Stmt>) -> Result<()> {
        for mut stmt in stmts {
            stmt.id = Some(self.id.gen());
            if let Some(span) = stmt.span {
                self.context.span_map.insert(stmt.id.unwrap(), span);
            }

            let ident = Ident {
                path: self.current_module_path.clone(),
                name: stmt.name().to_string(),
            };

            let stmt_name = stmt.name().to_string();

            let mut def = match stmt.kind {
                StmtKind::QueryDef(d) => {
                    let decl = DeclKind::QueryDef(*d);
                    self.context
                        .declare(ident, decl, stmt.id, Vec::new())
                        .with_span(stmt.span)?;
                    continue;
                }
                StmtKind::VarDef(var_def) => self.fold_var_def(var_def)?,
                StmtKind::TypeDef(ty_def) => {
                    let mut value = if let Some(value) = ty_def.value {
                        value
                    } else {
                        Box::new(Expr::new(Literal::Null))
                    };

                    // This is a hacky way to provide values to std.int and friends.
                    if self.current_module_path == vec![NS_STD] {
                        if let Some(kind) = get_stdlib_decl(&ident.name) {
                            value.kind = kind;
                        }
                    }

                    let mut ty = self.fold_type_expr(Some(value))?.unwrap();
                    ty.name = Some(ident.name.clone());

                    VarDef {
                        name: ty_def.name,
                        value: Box::new(Expr::new(ExprKind::Type(ty))),
                        ty_expr: None,
                    }
                }
                StmtKind::ModuleDef(module_def) => {
                    if !self.options.allow_module_decls {
                        return Err(Error::new_simple(
                            "explicit module declarations are not allowed",
                        )
                        .with_span(stmt.span)
                        .into());
                    }

                    self.current_module_path.push(ident.name);

                    let decl = Decl {
                        declared_at: stmt.id,
                        kind: DeclKind::Module(Module {
                            names: HashMap::new(),
                            redirects: Vec::new(),
                            shadowed: None,
                        }),
                        ..Default::default()
                    };
                    let ident = Ident::from_path(self.current_module_path.clone());
                    self.context
                        .module
                        .insert(ident, decl)
                        .with_span(stmt.span)?;

                    self.fold_statements(module_def.stmts)?;
                    self.current_module_path.pop();
                    continue;
                }
            };

            if def.name == "main" {
                def.ty_expr = Some(Box::new(Expr::new(ExprKind::Ident(Ident::from_path(
                    vec!["std", "relation"],
                )))));
            }

            if let ExprKind::Func(closure) = &mut def.value.kind {
                if closure.name_hint.is_none() {
                    closure.name_hint = Some(ident.clone());
                }
            }

            let expected_ty = self.fold_type_expr(def.ty_expr)?;
            if expected_ty.is_some() {
                let who = || Some(stmt_name.clone());
                self.validate_type(&mut def.value, expected_ty.as_ref(), &who)?;
            }

            let decl = prepare_expr_decl(def.value);

            self.context
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
                    LineageColumn::All { .. } => TupleField::Wildcard(None),
                    LineageColumn::Single { name, .. } => {
                        TupleField::Single(name.as_ref().map(|n| n.name.clone()), None)
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

fn get_stdlib_decl(name: &str) -> Option<ExprKind> {
    let set = match name {
        "int" => PrimitiveSet::Int,
        "float" => PrimitiveSet::Float,
        "bool" => PrimitiveSet::Bool,
        "text" => PrimitiveSet::Text,
        "date" => PrimitiveSet::Date,
        "time" => PrimitiveSet::Time,
        "timestamp" => PrimitiveSet::Timestamp,
        "func" => {
            return Some(ExprKind::Type(Ty {
                kind: TyKind::Function(None),
                name: None,
            }))
        }
        "anytype" => {
            return Some(ExprKind::Type(Ty {
                kind: TyKind::Any,
                name: None,
            }))
        }
        _ => return None,
    };
    Some(ExprKind::Type(Ty {
        kind: TyKind::Primitive(set),
        name: None,
    }))
}
