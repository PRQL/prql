use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{Frame, FrameColumn};

pub const NS_FUNC: &str = "_func";
pub const NS_FRAME: &str = "_frame";

/// Maps from accessible names in some context to their declarations.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Scope {
    /// Mapping from idents to their declarations. For each namespace (table), a map from column names to their definitions
    /// "_param" is namespace of current function parameters
    /// "_func" is namespace of functions
    pub(super) namespaces: HashMap<String, HashMap<String, usize>>,
}

impl Scope {
    /// insert into a namespace, override other declarations with the same name
    pub(super) fn add<S1: ToString, S2: ToString>(&mut self, namespace: S1, name: S2, id: usize) {
        let namespace = self.namespaces.entry(namespace.to_string()).or_default();

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

    pub fn lookup(&mut self, namespace: &str, var_name: &str) -> HashSet<(String, usize)> {
        let mut res = HashSet::new();

        if !namespace.is_empty() {
            if let Some(table) = self.namespaces.get(namespace) {
                if let Some(id) = table.get(var_name) {
                    res.insert((namespace.to_string(), *id));
                }
            }
        } else {
            // no namespace specified: lookup in all namespaces
            for (namespace, table) in &self.namespaces {
                if let Some(id) = table.get(var_name) {
                    res.insert((namespace.clone(), *id));
                }
            }
        }

        res
    }

    // /// Retains only functions tables and drop frame columns and function params
    // pub(super) fn drop_non_global(&mut self) {
    //     self.namespaces.retain(|name, _| name == NS_FUNC);
    // }

    /// Drops a namespace
    pub(super) fn drop(&mut self, namespace: &str) -> HashMap<String, usize> {
        let ns = self.namespaces.remove(namespace);

        ns.unwrap_or_default()
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
        for (namespace, table) in namespaces {
            writeln!(f, "{namespace}:")?;
            let names: Vec<_> = table.iter().sorted_by_key(|k| k.0).collect();
            for (name, id) in names {
                writeln!(f, "  {name:+20}: {id}")?;
            }
        }
        Ok(())
    }
}
