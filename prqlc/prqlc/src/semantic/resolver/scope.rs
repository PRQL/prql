use indexmap::IndexMap;

use crate::pr::{Ident, Ty};
use crate::codegen;
use crate::ir::decl::{Decl, DeclKind};
use crate::semantic::{NS_LOCAL, NS_THAT, NS_THIS};
use crate::{Error, Result, WithErrorInfo};

use super::tuple::StepOwned;
use super::Resolver;

#[derive(Debug)]
pub(super) struct Scope {
    pub types: IndexMap<String, Decl>,

    pub values: IndexMap<String, Decl>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            types: IndexMap::new(),
            values: IndexMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Decl> {
        if let Some(decl) = self.types.get(name) {
            return Some(decl);
        }

        self.values.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Decl> {
        if let Some(decl) = self.types.get_mut(name) {
            return Some(decl);
        }

        self.values.get_mut(name)
    }
}

impl Resolver<'_> {
    /// Get declaration from within the current scope.
    ///
    /// Does not mutate the current scope or module structure.
    pub(super) fn get_ident(&self, ident: &Ident) -> Option<&Decl> {
        if ident.starts_with_part(NS_LOCAL) {
            assert!(ident.len() == 2);
            self.scopes.last()?.get(&ident.name)
        } else {
            self.root_mod.module.get(ident)
        }
    }

    /// Get mutable reference to a declaration from within the current scope.
    ///
    /// Does not mutate the current scope or module structure.
    pub(super) fn get_ident_mut(&mut self, ident: &Ident) -> Option<&mut Decl> {
        if ident.starts_with_part(NS_LOCAL) {
            assert!(ident.len() == 2);
            self.scopes.last_mut()?.get_mut(&ident.name)
        } else {
            self.root_mod.module.get_mut(ident)
        }
    }

    /// Performs an identifer lookup, possibly infering type information or
    /// even new declarations.
    pub(super) fn lookup_ident(&mut self, ident: &Ident) -> Result<LookupResult, Error> {
        if !ident.starts_with_part(NS_LOCAL) {
            // if ident is not local, it must have been resolved eariler
            // so we can just do a direct lookup
            return Ok(LookupResult::Direct);
        }
        assert!(ident.len() == 2);

        let res = if let Some(scope) = self.scopes.pop() {
            let r = self.lookup_in_scope(&scope, &ident.name);
            self.scopes.push(scope);
            r
        } else {
            Ok(None)
        };
        let mut res = res.and_then(|x| {
            x.ok_or_else(|| Error::new_simple(format!("Unknown name `{}`", &ident.name)))
        });

        if let Err(e) = &res {
            log::debug!(
                "cannot resolve `{}`: `{e:?}`,\nscope={:#?}",
                ident.name,
                self.scopes.last(),
            );

            // attach available names
            if let Some(this_ty) = self.get_ty_of_scoped_name(NS_THIS) {
                let this_ty = super::types::TypePreviewer::run(self, this_ty.clone());
                res = res.push_hint(format!("this = {}", codegen::write_ty(&this_ty)));
            }
            if let Some(that_ty) = self.get_ty_of_scoped_name(NS_THAT) {
                let that_ty = super::types::TypePreviewer::run(self, that_ty.clone());
                res = res.push_hint(format!("that = {}", codegen::write_ty(&that_ty)));
            }
        }
        res
    }

    fn lookup_in_scope(&mut self, scope: &Scope, name: &str) -> Result<Option<LookupResult>> {
        if scope.get(name).is_some() {
            return Ok(Some(LookupResult::Direct));
        }

        for (param_name, decl) in &scope.values {
            let DeclKind::Variable(Some(var_ty)) = &decl.kind else {
                continue;
            };

            let Some(steps) = self.lookup_name_in_tuple(var_ty, name)? else {
                continue;
            };

            return Ok(Some(LookupResult::Indirect {
                real_name: param_name.clone(),
                indirections: steps,
            }));
        }
        Ok(None)
    }

    fn get_ty_of_scoped_name(&self, name: &str) -> Option<&Ty> {
        let scope = self.scopes.last()?;

        let self_decl = scope.values.get(name)?;
        let self_ty = self_decl.kind.as_variable()?;
        self_ty.as_ref()
    }
}

/// When doing a lookup of, for example, `a` it might turn out that what we are
/// looking for is under fully-qualified path `this.b.a`. In such cases, lookup will
/// return an "indirect result". In this example, it would be
/// `Indirect { real_name: "this", indirections: vec!["b", "a"] }`.
pub enum LookupResult {
    Direct,
    Indirect {
        real_name: String,
        indirections: Vec<StepOwned>,
    },
}
