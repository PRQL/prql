use std::collections::HashSet;

use crate::ast::{Ident, Ty};
use crate::codegen;
use crate::ir::decl::DeclKind;
use crate::semantic::{
    NS_GENERIC, NS_INFER, NS_INFER_MODULE, NS_LOCAL, NS_PARAM, NS_SELF, NS_THAT, NS_THIS,
};
use crate::{Error, Result, WithErrorInfo};

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
                        "cannot resolve `{ident}`: `{e:?}`,\nthis={:#?}\nthat={:#?}\n_param={:#?}\n_generic={:#?}\n_generic={:#?}",
                        self.root_mod.local().names.get(NS_THIS),
                        self.root_mod.local().names.get(NS_THAT),
                        self.root_mod.local().names.get(NS_PARAM),
                        self.root_mod.local().names.get(NS_GENERIC),
                        self.root_mod.module.names.get(NS_GENERIC),
                    );
                } else {
                    log::debug!(
                        "cannot resolve `{ident}`: `{e:?}`, root_mod={:#?}",
                        self.root_mod.module
                    );
                }

                // attach available names
                if let Some(this_ty) = self.get_local_self_ty(NS_THIS) {
                    let this_ty = super::types::TypePreviewer::run(self, this_ty.clone());
                    res = res.push_hint(format!("this = {}", codegen::write_ty(&this_ty)));
                }
                if let Some(that_ty) = self.get_local_self_ty(NS_THAT) {
                    let that_ty = super::types::TypePreviewer::run(self, that_ty.clone());
                    res = res.push_hint(format!("that = {}", codegen::write_ty(&that_ty)));
                }
            }
        }
        res
    }

    fn get_local_self_ty(&self, mod_name: &str) -> Option<&Ty> {
        let module = self.root_mod.local().names.get(mod_name)?;
        let module = module.kind.as_module()?;

        let self_decl = module.names.get(NS_SELF)?;
        let self_ty = self_decl.kind.as_variable()?;
        self_ty.as_ref()
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
