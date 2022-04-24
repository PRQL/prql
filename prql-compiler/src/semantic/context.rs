use anyhow::Result;
use enum_as_inner::EnumAsInner;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::ast::*;
use crate::error::Span;

const NS_PARAM: &str = "_param";
const NS_GLOB: &str = "_glob";

/// Scope within which we can reference variables, functions and tables
/// Provides fast lookups for different names.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// current table columns (result of last pipeline)
    pub frame: Frame,

    /// Mapping from idents to their declarations. For each namespace (table), a map from column names to their definitions
    /// "_param" is namespace of current function parameters
    /// "_glob" is namespace of functions without parameters (global variables)
    pub(crate) variables: HashMap<String, HashSet<usize>>,

    pub(crate) functions: HashMap<String, usize>,

    /// All declarations, even those out of scope
    pub(crate) declarations: Vec<(Declaration, Option<Span>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<TableColumn>,
    pub sort: Vec<ColumnSort<usize>>,
    pub group: Vec<usize>,

    pub tables: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TableColumn {
    All(usize),
    Unnamed(usize),
    Named(String, usize),
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

impl Frame {
    pub fn groups_to_columns(&mut self) {
        for col in &self.group {
            self.columns.push(TableColumn::Unnamed(*col))
        }
    }

    pub fn decls_in_use(&self) -> HashSet<usize> {
        let mut r = HashSet::new();
        for col in &self.columns {
            match col {
                TableColumn::Unnamed(id) | TableColumn::Named(_, id) => {
                    r.insert(*id);
                }
                _ => {}
            }
        }
        for col in &self.group {
            r.insert(*col);
        }
        r
    }

    pub fn get_column_names(&self, context: &Context) -> Vec<Option<String>> {
        self.columns
            .iter()
            .map(|col| match col {
                TableColumn::All(namespace) => {
                    let (table, _) = &context.declarations[*namespace];
                    let table = table.as_table().map(|x| x.as_str()).unwrap_or("");
                    Some(format!("{table}.*"))
                }
                TableColumn::Unnamed(_) => None,
                TableColumn::Named(name, _) => Some(name.clone()),
            })
            .collect()
    }
}

impl Context {
    pub(crate) fn replace_declaration(&mut self, id: usize, new_decl: Declaration) {
        let (decl, _) = self.declarations.get_mut(id).unwrap();
        *decl = new_decl;
    }

    pub(crate) fn replace_declaration_expr(&mut self, id: usize, expr: Node) {
        self.replace_declaration(id, Declaration::Expression(Box::new(expr)));
    }

    /// Removes all names from scopes, except functions and columns in frame.
    pub(super) fn clear_scope(&mut self) {
        let in_use = self.frame.decls_in_use();

        self.variables.retain(|name, decls| {
            if let Some(id) = decls.iter().find(|id| in_use.contains(id)).cloned() {
                decls.clear();
                decls.insert(id);
                true
            } else {
                name.starts_with(NS_GLOB)
            }
        });

        let to_cascade: Vec<_> = self.variables.keys().cloned().collect();

        for name in to_cascade {
            self.cascade_variable(name.as_str());
        }
    }

    pub fn print(&self) {
        for (i, (d, _)) in self.declarations.iter().enumerate() {
            match d {
                Declaration::Expression(v) => {
                    println!("[{i:3}]: expr  `{}`", v.item);
                }
                Declaration::ExternRef { table, variable } => {
                    println!("[{i:3}]: col   `{variable}` from table {table:?}");
                }
                Declaration::Table(name) => {
                    println!("[{i:3}]: table `{name}`");
                }
                Declaration::Function(f) => {
                    println!("[{i:3}]: func  `{}`", f.name);
                }
            }
        }
        print!("[");
        for t_col in &self.frame.columns {
            match t_col {
                TableColumn::All(ns) => {
                    print!(" {ns}.* ")
                }
                TableColumn::Named(name, id) => {
                    print!(" {name}:{id} ")
                }
                TableColumn::Unnamed(id) => {
                    print!(" {id} ")
                }
            }
        }
        println!("]");
    }

    pub fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push((dec, span));
        self.declarations.len() - 1
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();
        let no_params = func_def.named_params.is_empty() && func_def.positional_params.is_empty();

        let span = func_def.body.span;
        let id = self.declare(Declaration::Function(func_def), span);

        if no_params {
            let name = format!("{NS_GLOB}.{name}");
            self.add_to_scope(&name, id);
        } else {
            self.functions.insert(name, id);
        }

        id
    }

    pub fn declare_table(&mut self, t: &TableRef) {
        let name = t.alias.clone().unwrap_or_else(|| t.name.clone());
        let decl = Declaration::Table(name.clone());

        let table_id = self.declare(decl, None);
        self.frame.tables.push(table_id);

        let var_name = format!("{name}.*");
        self.add_to_scope(var_name.as_str(), table_id);

        let column = TableColumn::All(table_id);
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

        let name = format!("{NS_PARAM}.{name}");
        let id = self.declare(Declaration::Expression(decl), None);

        self.add_to_scope(&name, id);

        id
    }

    pub fn add_to_scope(&mut self, ident: &str, id: usize) {
        // insert into own namespace, override other declarations
        let decls = self.variables.entry(ident.to_string()).or_default();
        let overridden = decls.drain().next();
        decls.insert(id);

        // remove overridden columns from frame
        if let Some(overridden) = overridden {
            self.frame.columns.retain(|col| col != &overridden);
        }

        self.cascade_variable(ident);
    }

    // insert into lower namespaces, possibly creating ambiguities
    fn cascade_variable(&mut self, ident: &str) {
        let id = *self.variables[ident].iter().next().unwrap();

        let (_, var_name) = split_var_name(ident);

        let decls = self.variables.entry(var_name.to_string()).or_default();
        decls.insert(id);
    }

    pub fn lookup_variable(&mut self, ident: &str, span: Option<Span>) -> Result<usize, String> {
        let (namespace, variable) = split_var_name(ident);

        if let Some(decls) = self.variables.get(ident) {
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

            if let Some(decls) = self.variables.get(&all) {
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
                        self.add_to_scope(ident, id);

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
        if let Some(ns) = self.variables.get(variable) {
            r.extend(ns.clone());
        }
        if let Some(ns) = self.variables.get("*") {
            r.extend(ns.clone());
        }
        r
    }
}

/// Splits ident into namespaces and variable name
pub fn split_var_name(ident: &str) -> (&str, &str) {
    ident.rsplit_once('.').unwrap_or(("", ident))
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
            TableColumn::Unnamed(id) | TableColumn::Named(_, id) => id == other,
        }
    }
}
