use std::collections::HashMap;

use crate::ir::decl::{Decl, DeclKind, InferTarget, Module, RootModule};
use crate::ir::pl;
use crate::pr;
use crate::{Error, Result, Span};

use super::{NS_DEFAULT_DB, NS_INFER, NS_MAIN, NS_QUERY_DEF, NS_STD};

impl Module {
    pub fn singleton<S: ToString>(name: S, entry: Decl) -> Module {
        Module {
            names: HashMap::from([(name.to_string(), entry)]),
            ..Default::default()
        }
    }

    pub fn new_root() -> Module {
        // Each module starts with a default namespace that contains a wildcard
        // and the standard library.
        Module {
            names: HashMap::from([
                (
                    NS_DEFAULT_DB.to_string(),
                    Decl::from(DeclKind::Module(Module::new_database())),
                ),
                (NS_STD.to_string(), Decl::from(DeclKind::default())),
            ]),
            shadowed: None,
            redirects: vec![],
        }
    }

    pub fn new_database() -> Module {
        let names = HashMap::from([(
            NS_INFER.to_string(),
            Decl::from(DeclKind::Infer(InferTarget::Table)),
        )]);
        Module {
            names,
            ..Default::default()
        }
    }

    pub fn insert(&mut self, fq_ident: pr::Ident, decl: Decl) -> Result<Option<Decl>, Error> {
        if fq_ident.path.is_empty() {
            Ok(self.names.insert(fq_ident.name, decl))
        } else {
            let (top_level, remaining) = fq_ident.pop_front();
            let entry = self.names.entry(top_level).or_default();

            if let DeclKind::Module(inner) = &mut entry.kind {
                inner.insert(remaining.unwrap(), decl)
            } else {
                Err(Error::new_simple(
                    "path does not resolve to a module or a table",
                ))
            }
        }
    }

    pub fn get_mut(&mut self, ident: &pr::Ident) -> Option<&mut Decl> {
        let mut ns = self;

        for part in &ident.path {
            let entry = ns.names.get_mut(part);

            match entry {
                Some(Decl {
                    kind: DeclKind::Module(inner),
                    ..
                }) => {
                    ns = inner;
                }
                _ => return None,
            }
        }

        ns.names.get_mut(&ident.name)
    }

    /// Get namespace entry using a fully qualified ident.
    pub fn get(&self, fq_ident: &pr::Ident) -> Option<&Decl> {
        let mut ns = self;

        for part in fq_ident.path.iter() {
            let decl = ns.names.get(part)?;

            if let DeclKind::Module(inner) = &decl.kind {
                ns = inner;
            } else {
                return None;
            }
        }

        ns.names.get(&fq_ident.name)
    }

    pub fn get_submodule(&self, path: &[String]) -> Option<&Module> {
        let mut curr_mod = self;
        for step in path {
            let decl = curr_mod.names.get(step)?;
            curr_mod = decl.kind.as_module()?;
        }
        Some(curr_mod)
    }

    pub fn get_submodule_mut(&mut self, path: &[String]) -> Option<&mut Module> {
        let mut curr_mod = self;
        for step in path {
            let decl = curr_mod.names.get_mut(step)?;
            curr_mod = decl.kind.as_module_mut()?;
        }
        Some(curr_mod)
    }

    pub fn get_module_path(&self, path: &[String]) -> Option<Vec<&Module>> {
        let mut res = vec![self];
        for step in path {
            let decl = res.last().unwrap().names.get(step)?;
            let module = decl.kind.as_module()?;
            res.push(module);
        }

        Some(res)
    }

    pub fn shadow(&mut self, ident: &str) {
        let shadowed = self.names.remove(ident).map(Box::new);
        let entry = DeclKind::Module(Module {
            shadowed,
            ..Default::default()
        });
        self.names.insert(ident.to_string(), entry.into());
    }

    pub fn unshadow(&mut self, ident: &str) {
        if let Some(entry) = self.names.remove(ident) {
            let ns = entry.kind.into_module().unwrap();

            if let Some(shadowed) = ns.shadowed {
                self.names.insert(ident.to_string(), *shadowed);
            }
        }
    }

    pub fn as_decls(&self) -> Vec<(pr::Ident, &Decl)> {
        let mut r = Vec::new();
        for (name, decl) in &self.names {
            match &decl.kind {
                DeclKind::Module(module) => r.extend(
                    module
                        .as_decls()
                        .into_iter()
                        .map(|(inner, decl)| (pr::Ident::from_name(name) + inner, decl)),
                ),
                _ => r.push((pr::Ident::from_name(name), decl)),
            }
        }
        r
    }

    /// Recursively finds all declarations that end in suffix.
    pub fn find_by_suffix(&self, suffix: &str) -> Vec<pr::Ident> {
        let mut res = Vec::new();

        for (name, decl) in &self.names {
            if let DeclKind::Module(module) = &decl.kind {
                let nested = module.find_by_suffix(suffix);
                res.extend(nested.into_iter().map(|x| x.prepend(vec![name.clone()])));
                continue;
            }

            if name == suffix {
                res.push(pr::Ident::from_name(name));
            }
        }

        res
    }

    /// Recursively finds all declarations with an annotation that has a specific name.
    pub fn find_by_annotation_name(&self, annotation_name: &pr::Ident) -> Vec<pr::Ident> {
        let mut res = Vec::new();

        for (name, decl) in &self.names {
            if let DeclKind::Module(module) = &decl.kind {
                let nested = module.find_by_annotation_name(annotation_name);
                res.extend(nested.into_iter().map(|x| x.prepend(vec![name.clone()])));
            }

            let has_annotation = decl_has_annotation(decl, annotation_name);
            if has_annotation {
                res.push(pr::Ident::from_name(name));
            }
        }
        res
    }
}

fn decl_has_annotation(decl: &Decl, annotation_name: &pr::Ident) -> bool {
    for ann in &decl.annotations {
        if super::is_ident_or_func_call(&ann.expr, annotation_name) {
            return true;
        }
    }
    false
}

type HintAndSpan = (Option<String>, Option<Span>);

impl RootModule {
    /// Finds that main pipeline given a path to either main itself or its parent module.
    /// Returns main expr and fq ident of the decl.
    pub fn find_main_rel(&self, path: &[String]) -> Result<(&pl::Expr, pr::Ident), HintAndSpan> {
        let (decl, ident) = self.find_main(path).map_err(|x| (x, None))?;

        let span = decl
            .declared_at
            .and_then(|id| self.span_map.get(&id))
            .cloned();

        let decl = (decl.kind.as_expr())
            .filter(|e| e.ty.as_ref().unwrap().is_relation())
            .ok_or((Some(format!("{ident} is not a relational variable")), span))?;

        Ok((decl.as_ref(), ident))
    }

    pub fn find_main(&self, path: &[String]) -> Result<(&Decl, pr::Ident), Option<String>> {
        let mut tried_idents = Vec::new();

        // is path referencing the relational var directly?
        if !path.is_empty() {
            let ident = pr::Ident::from_path(path.to_vec());
            let decl = self.module.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        // is path referencing the parent module?
        {
            let mut path = path.to_vec();
            path.push(NS_MAIN.to_string());

            let ident = pr::Ident::from_path(path);
            let decl = self.module.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        Err(Some(format!(
            "Expected a declaration at {}",
            tried_idents.join(" or ")
        )))
    }

    pub fn find_query_def(&self, main: &pr::Ident) -> Option<&pr::QueryDef> {
        let ident = pr::Ident {
            path: main.path.clone(),
            name: NS_QUERY_DEF.to_string(),
        };

        let decl = self.module.get(&ident)?;
        decl.kind.as_query_def()
    }

    /// Finds all main pipelines.
    pub fn find_mains(&self) -> Vec<pr::Ident> {
        self.module.find_by_suffix(NS_MAIN)
    }

    /// Finds declarations that are annotated with a specific name.
    pub fn find_by_annotation_name(&self, annotation_name: &pr::Ident) -> Vec<pr::Ident> {
        self.module.find_by_annotation_name(annotation_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::pl;
    use crate::pr::Literal;

    #[test]
    fn test_module_shadow_unshadow() {
        let mut module = Module::default();

        let ident = pr::Ident::from_name("test_name");
        let expr: pl::Expr = pl::Expr::new(pl::ExprKind::Literal(Literal::Integer(42)));
        let decl: Decl = DeclKind::Expr(Box::new(expr)).into();

        module.insert(ident.clone(), decl.clone()).unwrap();

        module.shadow("test_name");
        assert!(module.get(&ident) != Some(&decl));

        module.unshadow("test_name");
        assert_eq!(module.get(&ident).unwrap(), &decl);
    }
}
