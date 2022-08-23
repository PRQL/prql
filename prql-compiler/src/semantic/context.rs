use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Debug;

use super::{split_var_name, Declaration, Declarations, Scope};
use crate::ast::*;
use crate::error::Span;

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) scope: Scope,

    /// All declarations, even those out of scope
    pub(crate) declarations: Declarations,
}

impl Context {
    pub fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push(dec, span)
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();

        let span = func_def.body.span;
        let id = self.declare(Declaration::Function(func_def), span);

        self.scope.add_function(name, id);

        id
    }

    pub fn declare_table(&mut self, name: Ident, alias: Option<String>) -> usize {
        let alias = alias.unwrap_or_else(|| name.clone());

        let table_id = self.declare(Declaration::Table(alias.clone()), None);

        self.scope.add(alias, "*", table_id);
        table_id
    }

    pub fn lookup_name(&mut self, name: &str, span: Option<Span>) -> Result<usize, String> {
        let (namespace, variable) = split_var_name(name);

        // lookup the name
        let decls = self.scope.lookup(namespace, name);

        match decls.len() {
            // no match: try match *
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap().1),

            // ambiguous
            _ => {
                let decls = decls
                    .into_iter()
                    .map(|d| self.declarations.get(d.1))
                    .map(|d| format!("`{d}`"))
                    .join(", ");
                return Err(format!(
                    "Ambiguous reference. Could be from either of {decls}"
                ));
            }
        }

        // this variable can be from a namespace that we don't know all columns of
        let decls = self.scope.lookup(namespace, "*");

        match decls.len() {
            0 => {
                dbg!(&self.scope);
                Err(format!("Unknown name `{name}`"))
            }

            // single match, great!
            1 => {
                let (namespace, table_id) = decls.into_iter().next().unwrap();

                // declare this variable as ExternRef
                let decl = Declaration::ExternRef {
                    table: Some(table_id),
                    variable: variable.to_string(),
                };
                let id = self.declare(decl, span);
                self.scope.add(namespace, name.to_string(), id);

                Ok(id)
            }

            // don't report ambiguous variable, database may be able to resolve them
            _ => {
                let decl = Declaration::ExternRef {
                    table: None,
                    variable: name.to_string(),
                };
                let id = self.declare(decl, span);

                Ok(id)
            }
        }
    }

    pub fn lookup_namespaces_of(&mut self, variable: &str) -> HashSet<usize> {
        let (ns, var_name) = split_var_name(variable);

        let mut r = HashSet::new();
        r.extend(self.scope.lookup(ns, var_name).into_iter().map(|d| d.1));
        r.extend(self.scope.lookup(ns, "*").into_iter().map(|d| d.1));
        r
    }

    /// Move top-level expressions into declarations and replace them with idents
    pub(super) fn extract_decls(&mut self, nodes: Vec<Expr>) -> Result<Vec<Expr>> {
        let mut res = Vec::with_capacity(nodes.len());
        for node in nodes {
            let alias = node.alias.clone();

            let expr = self.extract_decl(node)?;

            res.push(if let Some(alias) = alias {
                // introduce a new expression alias
                let mut ident = Expr::from(ExprKind::Ident(alias));
                ident.declared_at = Some(expr.declared_at.unwrap());
                ident
            } else {
                expr
            });
        }
        Ok(res)
    }

    fn extract_decl(&mut self, expr: Expr) -> Result<Expr> {
        Ok(match expr.kind {
            // keep existing ident
            ExprKind::Ident(_) => expr,

            // declare new expression so it can be references from FrameColumn
            _ => {
                let span = expr.span;
                let id = self.declare(Declaration::Expression(Box::from(expr)), span);

                let mut placeholder = Expr::from(ExprKind::Ident("<unnamed>".to_string()));
                placeholder.declared_at = Some(id);
                placeholder
            }
        })
    }

    pub fn get_column_names(&self, frame: &Frame) -> Vec<Option<String>> {
        frame
            .columns
            .iter()
            .map(|col| match col {
                FrameColumn::All(namespace) => {
                    let (table, _) = &self.declarations.decls[*namespace];
                    let table = table.as_table().map(|x| x.as_str()).unwrap_or("");
                    Some(format!("{table}.*"))
                }
                FrameColumn::Unnamed(_) => None,
                FrameColumn::Named(name, _) => Some(name.clone()),
            })
            .collect()
    }
}

impl Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Declarations:\n{:?}", self.declarations)?;
        writeln!(f, "Scope:\n{:?}", self.scope)
    }
}
