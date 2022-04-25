use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub const NS_PARAM: &str = "_param";
const NS_GLOB: &str = "_glob";

/// Maps from accessible names in some context to their declarations.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Scope {
    /// Mapping from idents to their declarations. For each namespace (table), a map from column names to their definitions
    /// "_param" is namespace of current function parameters
    /// "_glob" is namespace of functions without parameters (global variables)
    pub(super) variables: HashMap<String, HashSet<usize>>,

    pub(super) functions: HashMap<String, usize>,
}

impl Scope {
    pub(super) fn add(&mut self, ident: String, id: usize) {
        // insert into own namespace, override other declarations
        let decls = self.variables.entry(ident.clone()).or_default();
        decls.clear();
        decls.insert(id);

        self.cascade_variable(ident.as_str());        
    }

    pub(super) fn add_function(&mut self, name: String, id: usize, has_params: bool) {
        if has_params {
            self.functions.insert(name, id);
        } else {
            self.add(format!("{NS_GLOB}.{name}"), id);
        }
    }

    // insert into lower namespaces, possibly creating ambiguities
    pub(super) fn cascade_variable(&mut self, ident: &str) {
        let id = *self.variables[ident].iter().next().unwrap();

        let (_, var_name) = split_var_name(ident);

        let decls = self.variables.entry(var_name.to_string()).or_default();
        decls.insert(id);
    }

    pub(super) fn clear_except(&mut self, except: HashSet<usize>) {
        self.variables.retain(|name, decls| {
            if let Some(id) = decls.iter().find(|id| except.contains(id)).cloned() {
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
}

/// Splits ident into namespaces and variable name
pub fn split_var_name(ident: &str) -> (&str, &str) {
    ident.rsplit_once('.').unwrap_or(("", ident))
}
