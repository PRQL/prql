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
    pub frame: Frame,

    /// For each namespace (table), a map from column names to their definitions
    /// "$" is namespace of variables not belonging to any table (aliased, join using)
    /// "%" is namespace of functions without parameters (global variables)
    /// "_" is namespace of current function
    pub(crate) variables: HashMap<String, HashMap<String, usize>>,

    /// For each variable, a set of its possible namespaces
    pub(crate) inverse: HashMap<String, HashSet<String>>,

    /// Functions with parameters (name is duplicated, but that's not much overhead)
    pub(crate) functions: HashMap<String, usize>,

    /// table aliases
    pub(crate) namespaces: HashMap<String, String>,

    /// All declarations, even those out of scope
    pub(crate) declarations: Vec<(Declaration, Option<Span>)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<TableColumn>,
    pub sort: Vec<ColumnSort<usize>>,
    pub group: Vec<usize>,
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

impl Frame {
    pub fn groups_to_columns(&mut self) {
        for col in &self.group {
            self.columns.push(TableColumn::Declared(*col))
        }
    }

    pub fn decls_in_use(&self) -> HashSet<usize> {
        let mut r = HashSet::new();
        for col in &self.columns {
            if let TableColumn::Declared(id) = col {
                r.insert(*id);
            }
        }
        for col in &self.group {
            r.insert(*col);
        }
        r
    }
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
    pub(crate) fn replace_declaration(&mut self, id: usize, node: Node) {
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

    /// Removes variables from all scopes, except $ and %.
    pub(super) fn flatten_scope(&mut self) {
        // point all table aliases to $
        // for alias in self.tables.values_mut() {
        // *alias = "$".to_string();
        // }

        let in_use = self.frame.decls_in_use();

        // remove namespaces and collect all their variables to "current frame" namespace
        let mut current = self.variables.remove("$").unwrap_or_default();
        current.retain(|_, id| in_use.contains(id));
        self.variables.retain(|name, space| match name.as_str() {
            "%" | "_" | "$" => true,
            _ => {
                // redirect namespace to $
                self.namespaces.insert(name.clone(), "$".to_string());

                current.extend((space.drain()).filter(|(_, id)| in_use.contains(id)));
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
                self.namespaces.insert(name.clone(), table_name.to_string());
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
            self.namespaces.insert(t.name.clone(), alias.clone());
            alias.clone()
        } else {
            t.name.clone()
        };
        self.namespaces.remove(&name);

        self.variables.insert(name.clone(), Default::default());

        self.declare_unknown_columns(name.as_str());
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
    pub fn declare_unknown_columns(&mut self, namespace: &str) {
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
                let namespace = self.resolve_namespace(ns);

                self.frame.columns.push(TableColumn::All(namespace));
            } else {
                self.frame.columns.push(TableColumn::Declared(id));
            }
        }
    }

    fn resolve_namespace(&self, namespace: &str) -> String {
        let mut namespace = namespace;
        while let Some(t) = self.namespaces.get(namespace) {
            namespace = t;
        }
        namespace.to_string()
    }

    pub fn lookup_variable(&mut self, ident: &str) -> Result<Option<usize>, String> {
        let (namespace, variable) = split_var_name(ident);

        if variable == "*" {
            return Ok(None);
        }

        let mut namespace = namespace;

        // try to find the namespace
        if namespace.is_empty() {
            if let Some(ns) = self.lookup_namespace_of(variable)? {
                namespace = ns
            } else {
                // matched to *, but multiple possible namespaces
                // -> return None, treating this ident as raw
                return Ok(None);
            }
        }

        // resolve table alias
        let namespace = self.resolve_namespace(namespace);

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

    /// Finds a namespace of a variable.
    pub fn lookup_namespace_of(&self, variable: &str) -> Result<Option<&String>, String> {
        if let Some(ns) = self.inverse.get(variable) {
            // lookup the inverse index

            match ns.len() {
                0 => unreachable!("inverse index contains empty lists?"),

                // single match, great!
                1 => Ok(ns.iter().next()),

                // ambiguous
                _ => Err(format!(
                    "Ambiguous variable. Could be from either of {:?}",
                    ns
                )),
            }
        } else if let Some(ns) = self.inverse.get("*") {
            // this variable can be from a namespace that we don't know all columns of

            match ns.len() {
                0 => unreachable!("inverse index contains empty lists?"),

                // single match, great!
                1 => Ok(ns.iter().next()),

                // don't report ambiguous variable, database may be able to resolve them
                _ => Ok(None),
            }
        } else {
            Err(format!("Unknown variable `{variable}`"))
        }
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

pub fn split_var_name(ident: &str) -> (&str, &str) {
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
