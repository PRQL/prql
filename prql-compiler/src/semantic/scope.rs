use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{Frame, FrameColumn};

pub const NS_FUNC: &str = "_func";
pub const NS_FRAME: &str = "_frame";
pub const NS_PARAM: &str = "_param";

/// Maps from accessible names in some context to their declarations.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Scope {
    /// Mapping from names to their declarations. For each namespace (table), we store a map from column names to their definitions.
    /// Additionally, namespaces can be over-layed, hence the [Vec] that serves as a stack.
    pub(super) namespaces: HashMap<String, Vec<HashMap<String, usize>>>,
}

impl Scope {
    /// Insert a name into a namespace, override other declarations with the same name.
    /// If no such namespace exists, an empty one is created.
    pub(super) fn add<S1: ToString, S2: ToString>(&mut self, namespace: S1, name: S2, id: usize) {
        let namespace_stack = self.namespaces.entry(namespace.to_string()).or_default();

        if namespace_stack.is_empty() {
            namespace_stack.push(HashMap::new());
        }
        let namespace = namespace_stack.last_mut().unwrap();

        namespace.insert(name.to_string(), id);
    }

    pub(super) fn add_function(&mut self, name: String, id: usize) {
        self.add(NS_FUNC, name, id);
    }

    pub(super) fn add_frame_columns(&mut self, frame: &Frame) {
        for column in &frame.columns {
            match column {
                FrameColumn::All(table_id) => self.add(NS_FRAME, "*", *table_id),
                FrameColumn::Unnamed(_) => {} // you cannot reference unnamed columns, duh!
                FrameColumn::Named(name, id) => self.add(NS_FRAME, name, *id),
            }
        }
    }

    /// Searches lookup tables in the stack top-to-bottom
    pub fn lookup(&mut self, namespace: &str, var_name: &str) -> HashSet<(String, usize)> {
        let mut res = HashSet::new();

        if !namespace.is_empty() {
            if let Some(stack) = self.namespaces.get(namespace) {
                for table in stack.iter().rev() {
                    if let Some(id) = table.get(var_name) {
                        res.insert((namespace.to_string(), *id));
                    }
                }
            }
        } else {
            // no namespace specified: lookup in all namespaces
            for (namespace, stack) in &self.namespaces {
                for table in stack.iter().rev() {
                    if let Some(id) = table.get(var_name) {
                        res.insert((namespace.clone(), *id));
                    }
                }
            }
        }

        res
    }

    // /// Retains only functions tables and drop frame columns and function params
    // pub(super) fn drop_non_global(&mut self) {
    //     self.namespaces.retain(|name, _| name == NS_FUNC);
    // }

    /// Drops a namespace completely (including all over-layed scopes)
    pub(super) fn drop(&mut self, namespace: &str) {
        self.namespaces.remove(namespace);
    }

    pub(super) fn push_namespace(&mut self, namespace: &str) {
        let stack = self.namespaces.entry(namespace.to_string()).or_default();
        stack.push(HashMap::new());
    }

    pub(super) fn pop_namespace(&mut self, namespace: &str) -> Option<HashMap<String, usize>> {
        let stack = self.namespaces.entry(namespace.to_string()).or_default();
        stack.pop()
    }
}

/// Splits ident into namespaces and variable name
pub fn split_var_name(ident: &str) -> (&str, &str) {
    ident.rsplit_once('.').unwrap_or(("", ident))
}

impl Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "namespaces:")?;
        let namespaces: Vec<_> = self.namespaces.iter().sorted_by_key(|k| k.0).collect();
        for (namespace, table_stack) in namespaces {
            writeln!(f, "{namespace} ({}):", table_stack.len())?;
            if let Some(table) = table_stack.last() {
                let names: Vec<_> = table.iter().sorted_by_key(|k| k.0).collect();
                for (name, id) in names {
                    writeln!(f, "  {name:+20}: {id}")?;
                }
            }
        }
        Ok(())
    }
}
