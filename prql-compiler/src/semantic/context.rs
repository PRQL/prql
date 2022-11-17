use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use super::{Declaration, Declarations, Scope};
use crate::ast::*;
use crate::error::Span;

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) scope: Scope,

    /// All declarations, even those out of scope
    pub(crate) declarations: Declarations,

    /// Set of all ids that should be inlined during resolving
    pub(crate) inline: HashSet<usize>,
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

    pub fn declare_table(&mut self, name: String, alias: Option<String>) -> usize {
        let alias = alias.unwrap_or_else(|| name.clone());

        let table_id = self.declare(Declaration::Table(alias.clone()), None);

        self.scope.add(alias, "*", table_id);
        table_id
    }

    pub fn lookup_ident(&mut self, ident: &Ident, span: Option<Span>) -> Result<usize, String> {
        // lookup the name
        if ident.name != "*" {
            let decls = self.scope.lookup(ident);

            match decls.len() {
                // no match: try match *
                0 => {}

                // single match, great!
                1 => return Ok(decls.into_iter().next().unwrap().0),

                // ambiguous
                _ => {
                    let decls = decls
                        .into_iter()
                        .map(|d| self.declarations.get(d.0))
                        .map(|d| format!("`{d}`"))
                        .join(", ");
                    return Err(format!(
                        "Ambiguous reference. Could be from either of {decls}"
                    ));
                }
            }
        }

        // this variable can be from a namespace that we don't know all columns of
        let decls = self.scope.lookup(&Ident {
            path: ident.path.clone(),
            name: "*".to_string(),
        });

        match decls.len() {
            0 => Err(format!("Unknown name {ident:?}")),

            // single match, great!
            1 => {
                let (table_id, namespaces) = decls.into_iter().next().unwrap();

                // declare this variable as ExternRef
                let decl = Declaration::ExternRef {
                    table: Some(table_id),
                    variable: ident.name.clone(),
                };
                let id = self.declare(decl, span);
                for namespace in namespaces {
                    self.scope.add(namespace, &ident.name, id);
                }

                Ok(id)
            }

            // don't report ambiguous variable, database may be able to resolve them
            _ => {
                let decl = Declaration::ExternRef {
                    table: None,
                    variable: ident.name.clone(),
                };
                let id = self.declare(decl, span);

                Ok(id)
            }
        }
    }

    /// Ensure that expressions are declared.
    /// If expr is aliased, replace it with an ident.
    pub(super) fn declare_as_idents(&mut self, exprs: &mut [Expr]) {
        for expr in exprs {
            self.declare_as_ident(expr);
        }
    }

    /// Ensure that expression are declared.
    /// If expr is aliased, replace it with an ident.
    pub(super) fn declare_as_ident(&mut self, expr: &mut Expr) {
        // ensure that expr id declared
        expr.declared_at = expr.declared_at.or_else(|| {
            Some(self.declare(Declaration::Expression(Box::from(expr.clone())), expr.span))
        });
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

    pub fn take_decls(&mut self, namespace: &str) -> HashMap<String, Declaration> {
        let dropped = self.scope.pop_namespace(namespace);
        let mut res = HashMap::new();
        for (name, id) in dropped.unwrap_or_default() {
            let decl = self.declarations.take(id);
            self.inline.remove(&id);
            self.declarations.forget(id);
            res.insert(name, decl);
        }
        res
    }

    pub fn insert_decls(&mut self, namespace: &str, decls: HashMap<String, Declaration>) {
        for (name, dec) in decls {
            let id = self.declarations.push(dec, None);
            self.inline.insert(id);
            self.scope.add(namespace, name, id);
        }
    }
}

impl Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Declarations:\n{:?}", self.declarations)?;
        writeln!(f, "Scope:\n{:?}", self.scope)
    }
}
