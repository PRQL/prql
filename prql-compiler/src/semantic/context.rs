use anyhow::Result;
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Debug;

use super::scope::NS_PARAM;
use super::{split_var_name, Frame, FrameColumn, Scope};
use crate::ast::*;
use crate::error::Span;

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Current table columns (result of last pipeline)
    pub(crate) frame: Frame,

    /// Map of all accessible names (for each namespace)
    pub(crate) scope: Scope,

    /// All declarations, even those out of scope
    pub(crate) declarations: Vec<(Declaration, Option<Span>)>,
}

#[derive(Debug, EnumAsInner, Clone, Serialize, Deserialize, strum::Display)]
pub enum Declaration {
    Expression(Box<Node>),
    ExternRef {
        /// Table can be None if we are unable to determine from which table this column
        /// is from.
        table: Option<usize>,
        /// Full identifier when table is None, only variable name when table is known.
        variable: String,
    },
    Table(String),
    Function(FuncDef),
}

impl Context {
    pub(crate) fn replace_declaration(&mut self, id: usize, new_decl: Declaration) {
        let (decl, _) = self.declarations.get_mut(id).unwrap();
        *decl = new_decl;
    }

    pub(crate) fn replace_declaration_expr(&mut self, id: usize, expr: Node) {
        self.replace_declaration(id, Declaration::Expression(Box::new(expr)));
    }

    /// Removes all names from scope, except functions and columns in frame.
    pub(super) fn clear_scope(&mut self) {
        let in_use = self.frame.decls_in_use();
        self.scope.clear_except(in_use);
    }

    pub fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push((dec, span));
        self.declarations.len() - 1
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();
        let has_params =
            !func_def.named_params.is_empty() || !func_def.positional_params.is_empty();

        let span = func_def.body.span;
        let id = self.declare(Declaration::Function(func_def), span);

        self.scope.add_function(name, id, has_params);

        id
    }

    pub fn declare_table(&mut self, t: &TableRef) {
        let name = t.alias.clone().unwrap_or_else(|| t.name.clone());
        let decl = Declaration::Table(name.clone());

        let table_id = self.declare(decl, None);
        self.frame.tables.push(table_id);

        let var_name = format!("{name}.*");
        self.scope.add(var_name, table_id);

        let column = FrameColumn::All(table_id);
        self.frame.columns.push(column);
    }

    pub fn declare_func_param(&mut self, node: &Node) -> usize {
        let name = match &node.item {
            Item::Ident(ident) => ident.clone(),
            Item::NamedExpr(NamedExpr { name, .. }) => name.clone(),
            _ => unreachable!(),
        };

        // doesn't matter, will get overridden anyway
        let decl = Box::new(Item::Ident("".to_string()).into());

        let id = self.declare(Declaration::Expression(decl), None);

        self.scope.add(format!("{NS_PARAM}.{name}"), id);

        id
    }

    pub fn lookup_variable(&mut self, ident: &str, span: Option<Span>) -> Result<usize, String> {
        let (namespace, variable) = split_var_name(ident);

        if let Some(decls) = self.scope.variables.get(ident) {
            // lookup the inverse index

            match decls.len() {
                0 => unreachable!("inverse index contains empty lists?"),

                // single match, great!
                1 => Ok(decls.iter().next().cloned().unwrap()),

                // ambiguous
                _ => Err(format!(
                    "Ambiguous variable. Could be from either of {:?}",
                    decls
                )),
            }
        } else {
            let all = if namespace.is_empty() {
                "*".to_string()
            } else {
                format!("{namespace}.*")
            };

            if let Some(decls) = self.scope.variables.get(&all) {
                // this variable can be from a namespace that we don't know all columns of

                match decls.len() {
                    0 => unreachable!("inverse index contains empty lists?"),

                    // single match, great!
                    1 => {
                        let table_id = decls.iter().next().unwrap();

                        let decl = Declaration::ExternRef {
                            table: Some(*table_id),
                            variable: variable.to_string(),
                        };
                        let id = self.declare(decl, span);
                        self.scope.add(ident.to_string(), id);

                        Ok(id)
                    }

                    // don't report ambiguous variable, database may be able to resolve them
                    _ => {
                        let decl = Declaration::ExternRef {
                            table: None,
                            variable: ident.to_string(),
                        };
                        let id = self.declare(decl, span);

                        Ok(id)
                    }
                }
            } else {
                Err(format!("Unknown variable `{ident}`"))
            }
        }
    }

    pub fn lookup_namespaces_of(&mut self, variable: &str) -> HashSet<usize> {
        let mut r = HashSet::new();
        if let Some(ns) = self.scope.variables.get(variable) {
            r.extend(ns.clone());
        }
        if let Some(ns) = self.scope.variables.get("*") {
            r.extend(ns.clone());
        }
        r
    }
}

impl From<Declaration> for anyhow::Error {
    fn from(dec: Declaration) -> Self {
        // panic!("Unexpected declaration type: {dec:?}");
        anyhow::anyhow!("Unexpected declaration type: {dec:?}")
    }
}

impl PartialEq<usize> for FrameColumn {
    fn eq(&self, other: &usize) -> bool {
        match self {
            FrameColumn::All(_) => false,
            FrameColumn::Unnamed(id) | FrameColumn::Named(_, id) => id == other,
        }
    }
}

impl Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, (d, _)) in self.declarations.iter().enumerate() {
            match d {
                Declaration::Expression(v) => {
                    writeln!(f, "[{i:3}]: expr  `{}`", v.item)?;
                }
                Declaration::ExternRef { table, variable } => {
                    writeln!(f, "[{i:3}]: col   `{variable}` from table {table:?}")?;
                }
                Declaration::Table(name) => {
                    writeln!(f, "[{i:3}]: table `{name}`")?;
                }
                Declaration::Function(func) => {
                    writeln!(f, "[{i:3}]: func  `{}`", func.name)?;
                }
            }
        }
        write!(f, "{:?}", self.frame)
    }
}
