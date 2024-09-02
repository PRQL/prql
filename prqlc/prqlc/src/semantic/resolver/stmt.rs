use std::collections::HashMap;

use crate::pr::{Ty, TyKind};
use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::*;
use crate::semantic::{NS_GENERIC, NS_STD};
use crate::Result;

use super::types::TypeReplacer;

impl super::Resolver<'_> {
    /// Entry point to the resolver.
    /// fq_ident must point to an unresolved declaration.
    pub fn resolve_decl(&mut self, fq_ident: Ident) -> Result<()> {
        if !fq_ident.starts_with_part(NS_STD) {
            log::debug!("resolving decl {fq_ident}");
        }

        // take decl out of the module
        let mut decl = {
            let module = self.root_mod.module.get_submodule_mut(&fq_ident.path);
            module.unwrap().names.remove(&fq_ident.name).unwrap()
        };
        let stmt = decl.kind.into_unresolved().unwrap();
        self.debug_current_decl = fq_ident.clone();

        // resolve
        match stmt {
            StmtKind::QueryDef(d) => {
                decl.kind = DeclKind::QueryDef(*d);
            }
            StmtKind::ModuleDef(_) => {
                unreachable!("module def cannot be unresolved at this point")
                // it should have been converted into Module in resolve_decls::init_module_tree
            }
            StmtKind::VarDef(var_def) => {
                let def = self.fold_var_def(var_def)?;
                let expected_ty = def.ty;

                decl.kind = match def.value {
                    Some(mut def_value) => {
                        // var value is provided

                        // validate type
                        if expected_ty.is_some() {
                            let who = || Some(fq_ident.name.clone());
                            self.validate_expr_type(&mut def_value, expected_ty.as_ref(), &who)?;
                        }

                        // finalize global generics
                        if let Some(mapping) = self.finalize_global_generics() {
                            let ty = def_value.ty.unwrap();
                            def_value.ty = Some(TypeReplacer::on_ty(ty, mapping));
                        }

                        DeclKind::Expr(def_value)
                    }
                    None => {
                        // var value is not provided: treat this var as a param
                        let mut expr = Box::new(Expr::new(ExprKind::Param(fq_ident.name.clone())));
                        expr.ty = expected_ty;
                        DeclKind::Expr(expr)
                    }
                };
            }
            StmtKind::TypeDef(ty_def) => {
                let value = if let Some(value) = ty_def.value {
                    value
                } else {
                    Ty::new(TyKind::Tuple(vec![]))
                };

                let mut ty = fold_type_opt(self, Some(value))?.unwrap();
                ty.name = Some(fq_ident.name.clone());

                decl.kind = DeclKind::Ty(ty);
            }
            StmtKind::ImportDef(target) => {
                decl.kind = DeclKind::Import(target.name);
            }
        };

        // put decl back in
        {
            let module = self.root_mod.module.get_submodule_mut(&fq_ident.path);
            module.unwrap().names.insert(fq_ident.name, decl);
        }
        Ok(())
    }

    pub fn finalize_global_generics(&mut self) -> Option<HashMap<Ident, Ty>> {
        let generics = self.root_mod.module.names.get_mut(NS_GENERIC)?;
        let generics = generics.kind.as_module_mut()?;

        let mut type_mapping = HashMap::new();

        let mut new_generics = Vec::new();
        for (name, decl) in generics.names.drain() {
            if let DeclKind::GenericParam(Some((candidate, span))) = decl.kind {
                // TODO: reject GenericParam(None) with 'cannot infer type, add annotations'

                if candidate.kind.is_tuple() {
                    // don't finalize tuples because they might not be complete yet
                    new_generics.push((
                        name,
                        Decl {
                            kind: DeclKind::GenericParam(Some((candidate, span))),
                            ..decl
                        },
                    ));
                } else {
                    // finalize this generic
                    type_mapping.insert(
                        Ident::from_path(vec![NS_GENERIC.to_string(), name]),
                        candidate,
                    );
                }
            } else {
                new_generics.push((name, decl));
            }
        }
        generics.names.extend(new_generics);

        if type_mapping.is_empty() {
            None
        } else {
            Some(type_mapping)
        }
    }
}
