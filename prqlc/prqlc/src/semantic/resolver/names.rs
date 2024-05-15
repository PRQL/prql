use crate::ast::{Ident, Ty};
use crate::codegen;
use crate::ir::decl::Decl;
use crate::semantic::{NS_LOCAL, NS_THAT, NS_THIS};
use crate::{Error, Result, WithErrorInfo};

use super::scope::LookupResult;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn get_ident(&self, ident: &Ident, only_types: bool) -> Option<&Decl> {
        if ident.starts_with_part(NS_LOCAL) {
            assert!(ident.len() == 2);
            self.scopes.last()?.get(&ident.name, only_types)
        } else {
            self.root_mod.module.get(ident)
        }
    }

    pub(super) fn get_ident_mut(&mut self, ident: &Ident, only_types: bool) -> Option<&mut Decl> {
        if ident.starts_with_part(NS_LOCAL) {
            assert!(ident.len() == 2);
            self.scopes.last_mut()?.get_mut(&ident.name, only_types)
        } else {
            self.root_mod.module.get_mut(ident)
        }
    }

    pub(super) fn lookup_ident(
        &mut self,
        ident: &Ident,
        only_types: bool,
    ) -> Result<LookupResult, Error> {
        if ident.starts_with_part(NS_LOCAL) {
            assert!(ident.len() == 2);
            self.lookup_ident_local(&ident.name, only_types)
        } else {
            Ok(LookupResult::Direct)
        }
    }

    fn lookup_ident_local(&mut self, ident: &str, only_types: bool) -> Result<LookupResult, Error> {
        let mut res = self.lookup_ident_local_core(ident, only_types);

        if let Err(e) = &res {
            log::debug!(
                "cannot resolve `{ident}`: `{e:?}`,\nscope={:#?}",
                self.scopes.last(),
            );

            // attach available names
            if let Some(this_ty) = self.get_param_ty(NS_THIS) {
                let this_ty = super::types::TypePreviewer::run(self, this_ty.clone());
                res = res.push_hint(format!("this = {}", codegen::write_ty(&this_ty)));
            }
            if let Some(that_ty) = self.get_param_ty(NS_THAT) {
                let that_ty = super::types::TypePreviewer::run(self, that_ty.clone());
                res = res.push_hint(format!("that = {}", codegen::write_ty(&that_ty)));
            }
        }
        res
    }

    fn get_param_ty(&self, name: &str) -> Option<&Ty> {
        let scope = self.scopes.last()?;

        let self_decl = scope.params.get(name)?;
        let self_ty = self_decl.kind.as_variable()?;
        self_ty.as_ref()
    }

    pub(super) fn lookup_ident_local_core(
        &self,
        name: &str,
        only_types: bool,
    ) -> Result<LookupResult, Error> {
        // base case: direct lookup within each of the scopes
        for scope in self.scopes.iter().rev() {
            if let Some(r) = scope.lookup(name, only_types)? {
                return Ok(r);
            }
        }

        Err(Error::new_simple(format!("Unknown name `{name}`")))
    }
}
