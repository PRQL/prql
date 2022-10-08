use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub const NS_PARAM: &str = "_param";
pub const NS_FUNC: &str = "_func";

/// Maps from accessible names in some context to their declarations.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Scope {
    /// Mapping from idents to their declarations. For each namespace (table), a map from column names to their definitions
    /// "_param" is namespace of current function parameters
    /// "_func" is namespace of functions
    pub(super) variables: HashMap<String, HashSet<usize>>,
}

impl Scope {
    pub(super) fn add(&mut self, ident: String, id: usize) {
        // insert into own namespace, override other declarations
        let decls = self.variables.entry(ident.clone()).or_default();
        decls.clear();
        decls.insert(id);

        self.cascade_variable(ident.as_str());
    }

    pub(super) fn add_function(&mut self, name: String, id: usize) {
        self.add(format!("{NS_FUNC}.{name}"), id);
    }

    // insert into lower namespaces, possibly creating ambiguities
    pub(super) fn cascade_variable(&mut self, ident: &str) {
        let id = *self.variables[ident].iter().next().unwrap();

        let (_, var_name) = split_var_name(ident);

        let decls = self.variables.entry(var_name.to_string()).or_default();
        decls.insert(id);
    }

    /// Removes all names from scope, except functions and columns in frame.
    pub(super) fn clear(&mut self) {
        let mut to_remove = HashSet::<usize>::new();
        self.variables.retain(|name, decls| {
            let remove = name.starts_with(NS_PARAM) || name.ends_with(".*") || name == "*";
            if remove {
                to_remove.extend(decls.iter());
            }
            !remove
        });
        self.variables.retain(|_, decls| {
            decls.retain(|d| !to_remove.contains(d));
            !decls.is_empty()
        });
    }
}

/// Splits ident into namespaces and variable name
pub fn split_var_name(ident: &str) -> (&str, &str) {
    ident.rsplit_once('.').unwrap_or(("", ident))
}
