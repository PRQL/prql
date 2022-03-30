use anyhow::Result;
use enum_as_inner::EnumAsInner;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use strum_macros::Display;

use crate::ast::*;
use crate::error::Span;

#[derive(Clone)]
pub struct ResolvedQuery {
    // Func decls, tables and a pipeline
    pub nodes: Vec<Node>,

    // Scope we obtain after analyzing all nodes in AST
    pub context: Context,
}

/// Scope within which we can reference variables, functions and tables
/// Provides fast lookups for different names.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Context {
    // current table columns (result of last pipeline)
    pub(super) table: Vec<TableColumn>,

    // For each namespace (table), a list of its variables (columns)
    // "" is default namespace
    // "%" is namespace of functions without parameters
    pub(super) scopes: HashMap<String, HashMap<String, usize>>,

    // Functions with parameters (name is duplicated, but that's not much overhead)
    pub(super) functions: HashMap<String, usize>,

    // All declarations, even those out of scope
    pub(super) declarations: Vec<(Declaration, Option<Span>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableColumn {
    All,
    Declared(usize),
}

#[derive(Debug, EnumAsInner, Display, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Declaration {
    Variable(VarDec),
    Table(String),
    Function(FuncDef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VarDec {
    // index of the columns in the table
    pub position: Option<usize>,
    // the Node whose expr is equivalent to this variable
    pub declaration: Box<Node>,
    // for aliased columns and functions without arguments
    pub name: Option<String>,
}

impl Context {
    // pub fn get_table_columns(&self) -> Vec<VarDec> {
    //     self.table
    //         .iter()
    //         .filter_map(|id| self.declarations[*id].0.as_variable().cloned())
    //         .collect()
    // }

    /// Takes a declaration with minimal memory copying. A dummy node is left in place.
    pub(super) fn take_declaration(&mut self, id: usize) -> Option<Box<Node>> {
        let (decl, _) = self.declarations.get_mut(id).unwrap();
        let decl = decl.as_node_mut()?;

        let dummy: Node = Item::Expr(vec![]).into();
        let node = std::mem::replace(decl, Box::new(dummy));
        Some(node)
    }

    /// Takes a declaration with minimal memory copying. A dummy node is left in place.
    pub(super) fn put_declaration(&mut self, id: usize, node: Node) {
        let (decl, _) = self.declarations.get_mut(id).unwrap();
        let decl = decl.as_node_mut();

        if let Some(decl) = decl {
            *decl = Box::from(node);
        }
    }

    pub(super) fn clear_scope(&mut self) {
        let functions_without_params = self.scopes.get("%").cloned().unwrap_or_default();

        let default = self.scopes.entry("".to_string()).or_default();
        default.clear();
        default.extend(functions_without_params);

        self.table.clear();
    }

    fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push((dec, span));
        self.declarations.len() - 1
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();
        let is_variable = func_def.named_params.is_empty() && func_def.positional_params.is_empty();

        let span = Some(func_def.body.span);
        let id = self.declare(Declaration::Function(func_def), span);

        if is_variable {
            let name = format!("%.{name}");
            self.declare_variable(Some(&name), id, false);
        } else {
            self.functions.insert(name, id);
        }

        id
    }

    pub fn declare_table_column(&mut self, position: usize, node: &Node) {
        let position = Some(position);
        let var_dec = if let Some(named_expr) = node.item.as_named_expr() {
            VarDec {
                position,
                declaration: named_expr.expr.clone(),
                name: Some(named_expr.name.clone()),
            }
        } else {
            VarDec {
                position,
                declaration: Box::from(node.clone()),
                name: None,
            }
        };

        let name = var_dec.name.clone();
        let id = self.declare(Declaration::Variable(var_dec), Some(node.span));

        self.declare_variable(name.as_deref(), id, true);
    }

    pub fn declare_func_param(&mut self, node: &Node) -> usize {
        let name = match &node.item {
            Item::Ident(ident) => ident.clone(),
            Item::NamedExpr(NamedExpr { name, .. }) => name.clone(),
            _ => unreachable!(),
        };

        let var_dec = VarDec {
            position: None,
            declaration: Box::new(Item::Ident(name.clone()).into()), // doesn't matter, will get overridden anyway
            name: Some(name.clone()),
        };

        let id = self.declare(Declaration::Variable(var_dec), None);

        self.declare_variable(Some(&name), id, false);

        id
    }

    fn declare_variable(&mut self, name: Option<&str>, id: usize, in_table: bool) {
        let mut overridden = None;

        if let Some(name) = name {
            let (namespace, variable) = name.rsplit_once('.').unwrap_or(("", name));

            let default = self.scopes.entry("".to_string()).or_default();
            overridden = default.insert(variable.to_string(), id);

            if !namespace.is_empty() {
                let namespace = self.scopes.entry(namespace.to_string()).or_default();
                namespace.insert(variable.to_string(), id);
            }
        }

        if in_table {
            if let Some(overridden) = overridden {
                self.table.retain(|col| match col {
                    TableColumn::All => true,
                    TableColumn::Declared(id) => *id != overridden,
                });
            }
            self.table.push(TableColumn::Declared(id));
        }
    }

    pub fn lookup_variable(&mut self, ident: &str) -> Option<usize> {
        let (namespace, variable) = ident.rsplit_once('.').unwrap_or(("", ident));

        if let Some(ns) = self.scopes.get(namespace) {
            if let Some(decl_id) = ns.get(variable) {
                return Some(*decl_id);
            }
        }
        None
    }
}

impl Declaration {
    pub fn into_node(self) -> Result<Box<Node>, Self> {
        match self {
            Declaration::Variable(VarDec { declaration, .. }) => Ok(declaration),
            Declaration::Table(_) => Err(self),
            Declaration::Function(FuncDef { body, .. }) => Ok(body),
        }
    }

    pub fn as_node_mut(&mut self) -> Option<&mut Box<Node>> {
        match self {
            Declaration::Variable(VarDec { declaration, .. }) => Some(declaration),
            Declaration::Table(_) => None,
            Declaration::Function(FuncDef { body, .. }) => Some(body),
        }
    }

    pub fn as_name(&self) -> Option<&String> {
        match self {
            Declaration::Variable(VarDec { name, .. }) => name.as_ref(),
            Declaration::Table(name) => Some(name),
            Declaration::Function(FuncDef { name, .. }) => Some(name),
        }
    }
}

impl From<Declaration> for anyhow::Error {
    fn from(dec: Declaration) -> Self {
        panic!("Unexpected declaration type: {dec:?}");
        // anyhow::anyhow!("Unexpected declaration type: {dec:?}");
    }
}
