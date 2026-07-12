use std::collections::HashSet;

use itertools::Itertools;

use super::Resolver;
use crate::ir::decl::{Decl, DeclKind, Module};
use crate::ir::pl::{Expr, ExprKind};
use crate::pr::Ident;
use crate::pr::TyKind;
use crate::semantic::{NS_INFER, NS_INFER_MODULE, NS_SELF, NS_SHADOWING_COL, NS_THAT, NS_THIS};
use crate::Error;
use crate::Result;
use crate::WithErrorInfo;

impl Resolver<'_> {
    pub(super) fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, Error> {
        let mut res = if let Some(default_namespace) = self.default_namespace.clone() {
            self.resolve_ident_core(ident, Some(&default_namespace))
        } else {
            // Walk up the module hierarchy: start with the fully-qualified
            // path (current_module_path + ident), then strip the innermost
            // ancestor segment on each retry until the prefix is empty.
            let prefix_len = self.current_module_path.len();
            let mut full = ident.clone().prepend(self.current_module_path.clone());
            let mut res = self.resolve_ident_core(&full, None);
            for i in (0..prefix_len).rev() {
                if res.is_ok() {
                    break;
                }
                full.path.remove(i);
                res = self.resolve_ident_core(&full, None);
            }
            res
        };

        match &res {
            Ok(fq_ident) => {
                let decl = self.root_mod.module.get(fq_ident).unwrap();
                if let DeclKind::Import(target) = &decl.kind {
                    let target = target.clone();
                    return self.resolve_ident(&target);
                }
            }
            Err(e) => {
                log::debug!(
                    "cannot resolve `{ident}`: `{e:?}`, root_mod={:#?}",
                    self.root_mod
                );

                // attach available names
                let mut available_names = Vec::new();
                available_names.extend(self.collect_columns_in_module(NS_THIS));
                available_names.extend(self.collect_columns_in_module(NS_THAT));
                if !available_names.is_empty() {
                    let available_names = available_names.iter().map(Ident::to_string).join(", ");
                    res = res.push_hint(format!("available columns: {available_names}"));
                }
            }
        }
        res
    }

    fn collect_columns_in_module(&mut self, mod_name: &str) -> Vec<Ident> {
        let mut cols = Vec::new();

        let Some(module) = self.root_mod.module.names.get(mod_name) else {
            return cols;
        };

        let DeclKind::Module(this) = &module.kind else {
            return cols;
        };

        for (ident, decl) in this.as_decls().into_iter().sorted_by_key(|x| x.1.order) {
            if let DeclKind::Column(_) = decl.kind {
                // A column shadowing a relation's name is nested under
                // `NS_SHADOWING_COL`; present it under the relation's own name
                // rather than leaking the internal key.
                if ident.name == NS_SHADOWING_COL {
                    if let Some(parent) = ident.pop() {
                        cols.push(parent);
                    }
                } else {
                    cols.push(ident);
                }
            }
        }
        cols
    }

    pub(super) fn resolve_ident_core(
        &mut self,
        ident: &Ident,
        default_namespace: Option<&String>,
    ) -> Result<Ident, Error> {
        // special case: wildcard
        if ident.name == "*" {
            // TODO: we may want to raise an error if someone has passed `download*` in
            // an attempt to query for all `download` columns and expects to be able
            // to select a `download_2020_01_01` column later in the query. But
            // sometimes we want to query for `*.parquet` files, and give them an
            // alias. So we don't raise an error here, but if there's a way of
            // differentiating the cases, we can implement that.
            // if ident.name != "*" {
            //     return Err("Unsupported feature: advanced wildcard column matching".to_string());
            // }

            // For bare `*` (no prefix), prepend the default namespace so it
            // resolves like `this.*` within the current context.
            let wildcard_ident = match (ident.path.is_empty(), default_namespace) {
                (true, Some(ns)) => ident.clone().prepend(vec![ns.clone()]),
                _ => ident.clone(),
            };
            return self.resolve_ident_wildcard(&wildcard_ident);
        }

        // base case: direct lookup
        let decls = self.root_mod.module.lookup(ident);
        match decls.len() {
            // no match: try match *
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap()),

            // ambiguous
            _ => return Err(ambiguous_error(decls, None)),
        }

        let ident = if let Some(default_namespace) = default_namespace {
            let ident = ident.clone().prepend(vec![default_namespace.clone()]);

            let decls = self.root_mod.module.lookup(&ident);
            match decls.len() {
                // no match: try match *
                0 => ident,

                // single match, great!
                1 => return Ok(decls.into_iter().next().unwrap()),

                // ambiguous
                _ => return Err(ambiguous_error(decls, None)),
            }
        } else {
            ident.clone()
        };

        // fallback case: try to match with NS_INFER and infer the declaration
        // from the original ident.
        match self.resolve_ident_fallback(&ident, NS_INFER) {
            // The declaration and all needed parent modules were created
            // -> just return the fq ident
            Ok(inferred_ident) => Ok(inferred_ident),

            // Was not able to infer.
            Err(None) => Err(Error::new_simple(
                format!("Unknown name `{}`", &ident).to_string(),
            )),
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

    fn resolve_ident_wildcard(&mut self, ident: &Ident) -> Result<Ident, Error> {
        let ident_self = ident.clone().pop().ok_or_else(|| {
            Error::new_simple("Column wildcard `*` must be qualified, e.g. `table_name.*`")
        })? + Ident::from_name(NS_SELF);

        let decls = self.root_mod.module.lookup(&ident_self);
        log::trace!("resolve_ident_wildcard decls: {decls:?}");

        match decls.len() {
            1 => {
                let module_fq_self = decls.into_iter().next().unwrap();

                // Materialize into a tuple literal, containing idents.
                let fields = self.construct_wildcard_include(&module_fq_self);
                log::trace!("resolve_ident_wildcard fields: {fields:?}");

                // This is just a workaround to return an Expr from this function.
                // We wrap the expr into DeclKind::Expr and save it into the root module.
                let cols_expr = Expr {
                    flatten: true,
                    ..Expr::new(ExprKind::Tuple(fields))
                };
                let cols_expr = DeclKind::Expr(Box::new(cols_expr));
                let save_as = format!("_wildcard_match_{module_fq_self}");
                self.root_mod
                    .module
                    .names
                    .insert(save_as.to_string(), cols_expr.into());

                // Then we can return ident to that decl.
                return Ok(Ident::from_name(save_as));
            }
            0 => {} // fallthrough
            _ => return Err(ambiguous_error(decls, Some(&ident.name))),
        };

        // Second pass - look for an Expr referenced by the ident; if
        // it has an inferred ty that is relation-shaped, return that.
        let ident = ident.clone().pop().unwrap();
        let decls = self.root_mod.module.lookup(&ident);
        log::trace!("resolve_ident_wildcard pass 2 decls: {decls:?}");

        match decls.len() {
            1 => {
                let relation_fq = decls.into_iter().next().unwrap();

                // Retrieve the expr type
                let decl = self.root_mod.module.get(&relation_fq).unwrap();
                let decl_kind = decl.kind.clone();

                let ty_tuple = (|| {
                    let DeclKind::Expr(expr) = &decl_kind else {
                        return None;
                    };
                    let Some(ty) = &expr.ty else { return None };
                    let TyKind::Array(Some(ty)) = &ty.kind else {
                        return None;
                    };
                    let TyKind::Tuple(tup) = &ty.kind else {
                        return None;
                    };
                    Some(tup)
                })();

                if let (DeclKind::Expr(expr), Some(ty_tuple)) = (&decl_kind, ty_tuple) {
                    log::trace!("ty_tuple is {ty_tuple:#?}");

                    let prefix = relation_fq.iter().collect_vec();
                    let fields = self.construct_wildcard_from_lineage(&prefix, expr);
                    log::trace!("resolve_ident_wildcard pass 2 fields: {fields:?}");

                    // This is just a workaround to return an Expr from this function.
                    // We wrap the expr into DeclKind::Expr and save it into the root module.
                    let cols_expr = Expr {
                        flatten: true,
                        ..Expr::new(ExprKind::Tuple(fields))
                    };
                    let cols_expr = DeclKind::Expr(Box::new(cols_expr));
                    let save_as = format!("_wildcard_match_{relation_fq}");
                    self.root_mod
                        .module
                        .names
                        .insert(save_as.to_string(), cols_expr.into());

                    // Then we can return ident to that decl.
                    return Ok(Ident::from_name(save_as));
                }
            }
            0 => {} // fallthrough
            _ => return Err(ambiguous_error(decls, Some(&ident.name))),
        }

        Err(Error::new_simple(format!("Unknown relation {ident}")))
    }
}

fn ambiguous_error(idents: HashSet<Ident>, replace_name: Option<&String>) -> Error {
    let all_this = idents.iter().all(|d| d.starts_with_part(NS_THIS));

    let mut chunks = Vec::new();
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
            ident.name.clone_from(name);
        }
        chunks.push(ident.to_string());
    }
    chunks.sort();
    let hint = format!("could be any of: {}", chunks.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}
