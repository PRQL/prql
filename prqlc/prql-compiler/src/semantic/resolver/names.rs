use std::collections::HashSet;

use anyhow::Result;
use itertools::Itertools;

use prqlc_ast::expr::Ident;

use crate::ir::decl::{Decl, DeclKind, Module};
use crate::ir::pl::{Expr, ExprKind};
use crate::semantic::{NS_INFER, NS_INFER_MODULE, NS_SELF, NS_THAT, NS_THIS};
use crate::Error;
use crate::WithErrorInfo;

use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, Error> {
        if let Some(default_namespace) = self.default_namespace.clone() {
            self.resolve_ident_core(ident, Some(&default_namespace))
        } else {
            let mut ident = ident.clone().prepend(self.current_module_path.clone());

            let mut res = self.resolve_ident_core(&ident, None);
            for _ in 0..self.current_module_path.len() {
                if res.is_ok() {
                    break;
                }
                ident = ident.pop_front().1.unwrap();
                res = self.resolve_ident_core(&ident, None);
            }
            res
        }

        // log::debug!(
        //     "cannot resolve `{ident}`: `{e}`, root_mod={:#?}",
        //     self.root_mod
        // );
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
            return self.resolve_ident_wildcard(ident).map_err(|e| {
                log::debug!("{:#?}", self.root_mod.module);
                Error::new_simple(e)
            });
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
            if let DeclKind::InstanceOf(table_ident) = decl.kind {
                log::debug!("inferring {original} to be from table {table_ident}");
                self.infer_table_column(&table_ident, &original.name)?;
            }
        }

        Ok(module_ident + Ident::from_name(original.name.clone()))
    }

    fn resolve_ident_wildcard(&mut self, ident: &Ident) -> Result<Ident, String> {
        // Try matching ident prefix with a module
        let (mod_ident, mod_decl) = {
            if ident.path.len() > 1 {
                // Ident has specified full path
                let mod_ident = ident.clone().pop().unwrap();
                let mod_decl = (self.root_mod.module.get_mut(&mod_ident))
                    .ok_or_else(|| format!("Unknown relation {ident}"))?;

                (mod_ident, mod_decl)
            } else {
                // Ident could be just part of NS_THIS
                let mod_ident = (Ident::from_name(NS_THIS) + ident.clone()).pop().unwrap();

                if let Some(mod_decl) = self.root_mod.module.get_mut(&mod_ident) {
                    (mod_ident, mod_decl)
                } else {
                    // ... or part of NS_THAT
                    let mod_ident = (Ident::from_name(NS_THAT) + ident.clone()).pop().unwrap();

                    let mod_decl = self.root_mod.module.get_mut(&mod_ident);

                    // ... well - I guess not. Throw.
                    let mod_decl = mod_decl.ok_or_else(|| format!("Unknown relation {ident}"))?;

                    (mod_ident, mod_decl)
                }
            }
        };

        // Unwrap module
        let module = (mod_decl.kind.as_module_mut())
            .ok_or_else(|| format!("Expected a module {mod_ident}"))?;

        let fq_cols = if module.names.contains_key(NS_INFER) {
            // Columns can be inferred, which means that we don't know all column names at
            // compile time: use ExprKind::All
            vec![Expr::new(ExprKind::All {
                within: mod_ident.clone(),
                except: Vec::new(),
            })]
        } else {
            // Columns cannot be inferred, what's in the namespace is all there
            // could be in this namespace.
            (module.names.iter())
                .filter(|(_, decl)| matches!(&decl.kind, DeclKind::Column(_)))
                .sorted_by_key(|(_, decl)| decl.order)
                .map(|(name, _)| mod_ident.clone() + Ident::from_name(name))
                .map(|fq_col| Expr::new(ExprKind::Ident(fq_col)))
                .collect_vec()
        };

        // This is just a workaround to return an Expr from this function.
        // We wrap the expr into DeclKind::Expr and save it into the root module.
        let cols_expr = Expr {
            flatten: true,
            ..Expr::new(ExprKind::Tuple(fq_cols))
        };
        let cols_expr = DeclKind::Expr(Box::new(cols_expr));
        let save_as = "_wildcard_match";
        module.names.insert(save_as.to_string(), cols_expr.into());

        // Then we can return ident to that decl.
        Ok(mod_ident + Ident::from_name(save_as))
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
            ident.name = name.clone();
        }
        chunks.push(ident.to_string());
    }
    chunks.sort();
    let hint = format!("could be any of: {}", chunks.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}
