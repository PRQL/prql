use anyhow::Result;
use enum_as_inner::EnumAsInner;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use strum::Display;

use crate::ast::*;
use crate::error::Span;

const DECL_ALL: usize = usize::MAX;

/// Scope within which we can reference variables, functions and tables
/// Provides fast lookups for different names.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// current table columns (result of last pipeline)
    pub(super) frame: Frame,

    /// For each namespace (table), a map from column names to their definitions
    /// "$" is namespace of variables not belonging to any table (aliased, join using)
    /// "%" is namespace of functions without parameters (global variables)
    /// "_" is namespace of current function
    pub(super) variables: HashMap<String, HashMap<String, usize>>,

    /// For each variable, a set of its possible namespaces
    pub(super) inverse: HashMap<String, HashSet<String>>,

    /// Functions with parameters (name is duplicated, but that's not much overhead)
    pub(super) functions: HashMap<String, usize>,

    /// table aliases
    pub(super) tables: HashMap<String, String>,

    /// All declarations, even those out of scope
    pub(super) declarations: Vec<(Declaration, Option<Span>)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<TableColumn>,
    pub sort: Vec<ColumnSort<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableColumn {
    All(String),
    Declared(usize),
}

#[derive(Debug, EnumAsInner, Display, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Declaration {
    Variable(Box<Node>),
    Table(String),
    Function(FuncDef),
}

impl Context {
    pub fn get_frame(&self) -> Vec<Option<String>> {
        self.frame
            .columns
            .iter()
            .map(|col| match col {
                TableColumn::All(namespace) => Some(format!("{namespace}.*")),
                TableColumn::Declared(id) => self.declarations[*id].0.as_name().cloned(),
            })
            .collect()
    }

    /// Takes a declaration with minimal memory copying. A dummy node is left in place.
    pub(super) fn replace_declaration(&mut self, id: usize, node: Node) {
        let (decl, _) = self.declarations.get_mut(id).unwrap();
        let decl = decl.as_node_mut();

        if let Some(decl) = decl {
            *decl = Box::from(node);
        }
    }

    /// Removes all variables from default variable scope, except for functions without params.
    pub(super) fn refresh_inverse_index(&mut self) {
        self.inverse.clear();

        for namespace in &self.variables {
            for variable in namespace.1 {
                let entry = self.inverse.entry(variable.0.clone()).or_default();
                entry.insert(namespace.0.clone());
            }
        }
    }

    /// Removes variables from all scopes, except $ and %. Also clears frame.
    pub(super) fn clear_scopes(&mut self) {
        // point all table aliases to $
        // for alias in self.tables.values_mut() {
        // *alias = "$".to_string();
        // }

        // remove namespaces and collect all their variables to "current frame" namespace
        let mut current = self.variables.remove("$").unwrap_or_default();
        current.retain(|_, id| self.frame.columns.iter().any(|c| c == id));
        self.variables.retain(|name, space| match name.as_str() {
            "%" | "_" | "$" => true,
            _ => {
                // redirect namespace to $
                self.tables.insert(name.clone(), "$".to_string());

                current.extend(
                    (space.drain()).filter(|(_, id)| self.frame.columns.iter().any(|c| c == id)),
                );
                false
            }
        });

        // insert back variables that are in frame
        self.variables.insert("$".to_string(), current);

        self.refresh_inverse_index();
    }

    pub fn finish_table(&mut self, table_name: &str) {
        self.variables.retain(|name, _| match name.as_str() {
            "_" | "$" | "%" => true,
            _ => {
                self.tables.insert(name.clone(), table_name.to_string());
                false
            }
        });
    }

    fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push((dec, span));
        self.declarations.len() - 1
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();
        let is_variable = func_def.named_params.is_empty() && func_def.positional_params.is_empty();

        let span = func_def.body.span;
        let id = self.declare(Declaration::Function(func_def), span);

        if is_variable {
            let name = format!("%.{name}");
            self.add_to_scope(Some(&name), id, false);
        } else {
            self.functions.insert(name, id);
        }

        id
    }

    pub fn declare_table(&mut self, t: &TableRef) {
        let name = if let Some(alias) = &t.alias {
            self.tables.insert(t.name.clone(), alias.clone());
            alias.clone()
        } else {
            t.name.clone()
        };
        self.tables.remove(&name);

        self.variables.insert(name.clone(), Default::default());

        self.declare_all_columns(name.as_str());
    }

    // pub fn rename_table(&mut self, old: &str, new: &str) {
    //     if let Some(old_ns) = self.variables.remove(old) {
    //         let new_ns = self.variables.entry(new.to_string()).or_default();
    //         new_ns.extend(old_ns);
    //     }
    //     // self.tables.insert(old.to_string(), new.to_string());

    //     for (_, namespaces) in &mut self.inverse {
    //         if namespaces.remove(old) {
    //             namespaces.insert(new.to_string());
    //         }
    //     }
    // }

    pub fn declare_table_column(&mut self, node: &Node, in_frame: bool) -> usize {
        let decl = Declaration::Variable(Box::from(node.clone()));

        let name = decl.as_name().cloned();
        let id = self.declare(decl, node.span);

        self.add_to_scope(name.as_deref(), id, in_frame);
        id
    }

    /// Puts "*" in scope
    ///
    /// Does not actually declare anything.
    pub fn declare_all_columns(&mut self, namespace: &str) {
        let name = format!("{namespace}.*");
        self.add_to_scope(Some(name.as_str()), DECL_ALL, true);
    }

    pub fn declare_func_param(&mut self, node: &Node) -> usize {
        let name = match &node.item {
            Item::Ident(ident) => ident.clone(),
            Item::NamedExpr(NamedExpr { name, .. }) => name.clone(),
            _ => unreachable!(),
        };

        // doesn't matter, will get overridden anyway
        let decl = Box::new(Item::Ident(name.clone()).into());

        let name = format!("_.{name}");
        let id = self.declare(Declaration::Variable(decl), None);

        self.add_to_scope(Some(&name), id, false);

        id
    }

    fn add_to_scope(&mut self, name: Option<&str>, id: usize, in_frame: bool) {
        let name = name.map(split_var_name);

        if let Some((namespace, variable)) = name {
            let namespace = if namespace.is_empty() { "$" } else { namespace };

            // insert into own namespace
            let own = self.variables.entry(namespace.to_string()).or_default();
            let overridden = own.insert(variable.to_string(), id);

            // insert into default namespace
            let default = (self.inverse.entry(variable.to_string())).or_default();
            default.insert(namespace.to_string());

            // remove overridden columns from frame
            if let Some(overridden) = overridden {
                self.frame.columns.retain(|col| col != &overridden);
            }
        }

        // add column to frame
        if in_frame {
            if let Some((ns, "*")) = name {
                let mut namespace = ns.to_string();
                while let Some(ns) = self.tables.get(&namespace) {
                    namespace = ns.clone();
                }

                self.frame.columns.push(TableColumn::All(namespace));
            } else {
                self.frame.columns.push(TableColumn::Declared(id));
            }
        }
    }

    pub fn lookup_variable(&mut self, ident: &str) -> Result<Option<usize>, String> {
        let (namespace, variable) = split_var_name(ident);

        if variable == "*" {
            return Ok(None);
        }

        let mut namespace = namespace.to_string();

        // try to find the namespace
        if namespace.is_empty() {
            namespace = if let Some(ns) = self.lookup_namespace_of(variable)? {
                ns
            } else {
                // matched to *, but multiple possible namespaces
                // -> return None, treating this ident as raw
                return Ok(None);
            }
        }

        // resolve table alias
        while let Some(ns) = self.tables.get(&namespace) {
            namespace = ns.clone();
        }

        let ns = (self.variables.get(&namespace))
            .ok_or_else(|| format!("Unknown table `{namespace}`"))?;

        if let Some(decl_id) = ns.get(variable) {
            // variable found, return

            Ok(Some(*decl_id))
        } else if ns.get("*").is_some() {
            // because of "*", declare new ident "namespace.variable"

            let ident = Item::Ident(format!("{namespace}.{variable}")).into();
            let id = self.declare_table_column(&ident, false);

            Ok(Some(id))
        } else {
            Err(format!("Unknown variable `{namespace}.{variable}`"))
        }
    }

    pub fn lookup_namespace_of(&mut self, variable: &str) -> Result<Option<String>, String> {
        if let Some(ns) = self.inverse.get(variable) {
            if ns.len() == 1 {
                return Ok(ns.iter().next().cloned());
            }

            if ns.len() > 1 {
                return Err(format!(
                    "Ambiguous variable. Could be from either of {:?}",
                    ns
                ));
            }
        } else if let Some(ns) = self.inverse.get("*") {
            if ns.len() == 1 {
                return Ok(ns.iter().next().cloned());
            }
            // don't report ambiguous variable, database may be able to resolve them
            if ns.len() > 1 {
                return Ok(None);
            }
        }
        Err(format!("Unknown variable `{variable}`"))
    }

    pub fn lookup_namespaces_of(&mut self, variable: &str) -> HashSet<String> {
        let mut r = HashSet::new();
        if let Some(ns) = self.inverse.get(variable) {
            r.extend(ns.clone());
        }
        if let Some(ns) = self.inverse.get("*") {
            r.extend(ns.clone());
        }
        r
    }
}

pub(super) fn split_var_name(ident: &str) -> (&str, &str) {
    ident.rsplit_once('.').unwrap_or(("", ident))
}

impl Declaration {
    pub fn into_expr_node(self) -> Result<Box<Node>, Self> {
        match self {
            Declaration::Variable(node) => Ok(match node.item {
                Item::NamedExpr(named_expr) => named_expr.expr,
                _ => node,
            }),
            Declaration::Table(_) => Err(self),
            Declaration::Function(FuncDef { body, .. }) => Ok(body),
        }
    }

    pub fn as_node_mut(&mut self) -> Option<&mut Box<Node>> {
        match self {
            Declaration::Variable(declaration) => Some(declaration),
            Declaration::Table(_) => None,
            Declaration::Function(FuncDef { body, .. }) => Some(body),
        }
    }

    pub fn as_name(&self) -> Option<&String> {
        match self {
            Declaration::Variable(node) => match &node.item {
                Item::NamedExpr(named_expr) => Some(&named_expr.name),

                // if this is an identifier, use it as a name
                Item::Ident(ident) => Some(ident),

                // everything else is unnamed,
                _ => None,
            },
            Declaration::Table(name) => Some(name),
            Declaration::Function(FuncDef { name, .. }) => Some(name),
        }
    }
}

impl From<Declaration> for anyhow::Error {
    fn from(dec: Declaration) -> Self {
        // panic!("Unexpected declaration type: {dec:?}");
        anyhow::anyhow!("Unexpected declaration type: {dec:?}")
    }
}

impl PartialEq<usize> for TableColumn {
    fn eq(&self, other: &usize) -> bool {
        match self {
            TableColumn::All(_) => false,
            TableColumn::Declared(id) => id == other,
        }
    }
}
