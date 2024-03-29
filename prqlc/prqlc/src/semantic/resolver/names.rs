use std::collections::HashSet;

use itertools::Itertools;

use crate::Result;

use crate::ast::Ident;

use crate::ir::decl::{Decl, DeclKind, Module};
use crate::semantic::{
    NS_GENERIC, NS_INFER, NS_INFER_MODULE, NS_LOCAL, NS_PARAM, NS_SELF, NS_THAT, NS_THIS,
};
use crate::Error;
use crate::WithErrorInfo;

use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, Error> {
        let mut res = self.resolve_ident_core(ident);

        match &res {
            Ok(fq_ident) => {
                // handle imports
                let decl = self.root_mod.module.get(fq_ident).unwrap();
                if let DeclKind::Import(target) = &decl.kind {
                    let target = target.clone();
                    return self.resolve_ident(&target);
                }
            }
            Err(e) => {
                if ident.iter().next().unwrap() == NS_LOCAL {
                    log::debug!(
                        "cannot resolve `{ident}`: `{e:?}`,\nthis={:#?}\nthat={:#?}\n_param={:#?}\n_generic={:#?}",
                        self.root_mod.local().names.get(NS_THIS),
                        self.root_mod.local().names.get(NS_THAT),
                        self.root_mod.local().names.get(NS_PARAM),
                        self.root_mod.local().names.get(NS_GENERIC),
                    );
                } else {
                    log::debug!(
                        "cannot resolve `{ident}`: `{e:?}`, root_mod={:#?}",
                        self.root_mod.module
                    );
                }

                // attach available names
                let mut available_names = Vec::new();
                available_names.extend(self.collect_columns_in_a_local_module(NS_THIS));
                available_names.extend(self.collect_columns_in_a_local_module(NS_THAT));
                if !available_names.is_empty() {
                    let available_names = available_names.iter().map(Ident::to_string).join(", ");
                    res = res.push_hint(format!("available columns: {available_names}"));
                }
            }
        }
        res
    }

    fn collect_columns_in_a_local_module(&mut self, mod_name: &str) -> Vec<Ident> {
        let mut cols = Vec::new();

        let Some(module) = self.root_mod.local().names.get(mod_name) else {
            return cols;
        };

        let DeclKind::Module(this) = &module.kind else {
            return cols;
        };

        for (ident, decl) in this.as_decls().into_iter().sorted_by_key(|x| x.1.order) {
            if let DeclKind::TupleField(_) = decl.kind {
                cols.push(ident);
            }
        }
        cols
    }

    pub(super) fn resolve_ident_core(&mut self, ident: &Ident) -> Result<Ident, Error> {
        // base case: direct lookup
        let decls = self.root_mod.module.lookup(ident);
        match decls.len() {
            // no match: pass though
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap()),

            // ambiguous
            _ => return Err(ambiguous_error(decls, None)),
        }

        // fallback case: try to match with NS_INFER and infer the declaration
        // from the original ident.
        match self.resolve_ident_fallback(ident, NS_INFER) {
            // The declaration and all needed parent modules were created
            // -> just return the fq ident
            Ok(inferred_ident) => Ok(inferred_ident),

            // Was not able to infer.
            Err(None) => Err(Error::new_simple(format!("Unknown name `{ident}`"))),
            Err(Some(msg)) => Err(msg),
        }
    }

    /// Try lookup of the ident with name replaced. If unsuccessful, recursively retry parent ident.
    fn resolve_ident_fallback(
        &mut self,
        ident: &Ident,
        name_replacement: &'static str,
    ) -> Result<Ident, Option<Error>> {
        let infer_ident = ident.clone().with_name(name_replacement);

        // lookup of infer_ident
        let mut decls = self.root_mod.module.lookup(&infer_ident);

        if decls.is_empty() {
            if let Some(parent) = infer_ident.clone().pop() {
                // try to infer parent
                let _ = self.resolve_ident_fallback(&parent, NS_INFER_MODULE)?;

                // module was successfully inferred, retry the lookup
                decls = self.root_mod.module.lookup(&infer_ident)
            }
        }

        match decls.len() {
            1 => {
                // single match, great!
                let infer_ident = decls.into_iter().next().unwrap();
                self.infer_decl(infer_ident, ident)
                    .map_err(|x| Some(Error::new_simple(x)))
            }
            0 => Err(None),
            _ => Err(Some(ambiguous_error(decls, Some(&ident.name)))),
        }
    }

    /// Create a declaration of [original] from template provided by declaration of [infer_ident].
    fn infer_decl(&mut self, infer_ident: Ident, original: &Ident) -> Result<Ident, String> {
        let infer = self.root_mod.module.get(&infer_ident).unwrap();
        let mut infer_default = *infer.kind.as_infer().cloned().unwrap();

        if let DeclKind::Module(new_module) = &mut infer_default {
            // Modules are inferred only for database inference.
            // Because we want to infer database modules that nested arbitrarily deep,
            // we cannot store the template in DeclKind::Infer, but we override it here.
            *new_module = Module::new_database();
        }

        let module_ident = infer_ident.pop().unwrap();
        let module = self.root_mod.module.get_mut(&module_ident).unwrap();
        let module = module.kind.as_module_mut().unwrap();

        // insert default
        module
            .names
            .insert(original.name.clone(), Decl::from(infer_default));

        // infer table columns
        if let Some(decl) = module.names.get(NS_SELF).cloned() {
            if let DeclKind::InstanceOf(table_ident, _) = decl.kind {
                log::debug!("inferring {original} to be from table {table_ident}");
                self.infer_table_column(&table_ident, &original.name)?;
            }
        }

        Ok(module_ident + Ident::from_name(original.name.clone()))
    }
}

fn ambiguous_error(idents: HashSet<Ident>, replace_name: Option<&String>) -> Error {
    let all_this = idents.iter().all(|d| d.starts_with_part(NS_THIS));

    let mut candidates = Vec::new();
    for mut ident in idents {
        if all_this {
            let (_, rem) = ident.pop_front();
            if let Some(rem) = rem {
                ident = rem;
            } else {
                continue;
            }
        }

        if let Some(name) = replace_name {
            ident.name = name.clone();
        }
        candidates.push(ident.to_string());
    }
    candidates.sort();
    let hint = format!("could be any of: {}", candidates.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}
