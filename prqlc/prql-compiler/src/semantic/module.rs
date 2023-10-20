use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::ir::pl::{Expr, Ident, TupleField, Ty};
use crate::Error;

use super::{Lineage, LineageColumn};
use super::{NS_INFER, NS_INFER_MODULE, NS_SELF};
use crate::ir::decl::{Decl, DeclKind, Module, TableDecl, TableExpr};

impl Module {
    pub fn singleton<S: ToString>(name: S, entry: Decl) -> Module {
        Module {
            names: HashMap::from([(name.to_string(), entry)]),
            ..Default::default()
        }
    }

    pub fn new_database() -> Module {
        let names = HashMap::from([
            (
                NS_INFER.to_string(),
                Decl::from(DeclKind::Infer(Box::new(DeclKind::TableDecl(TableDecl {
                    ty: Some(Ty::relation(vec![TupleField::Wildcard(None)])),
                    expr: TableExpr::LocalTable,
                })))),
            ),
            (
                NS_INFER_MODULE.to_string(),
                Decl::from(DeclKind::Infer(Box::new(DeclKind::Module(Module {
                    names: HashMap::new(),
                    redirects: vec![],
                    shadowed: None,
                })))),
            ),
        ]);
        Module {
            names,
            shadowed: None,
            redirects: vec![],
        }
    }

    pub fn insert(&mut self, fq_ident: Ident, decl: Decl) -> Result<Option<Decl>, Error> {
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

    pub fn get_mut(&mut self, ident: &Ident) -> Option<&mut Decl> {
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
    pub fn get(&self, fq_ident: &Ident) -> Option<&Decl> {
        let mut ns = self;

        for (index, part) in fq_ident.path.iter().enumerate() {
            let decl = ns.names.get(part);
            if let Some(decl) = decl {
                match &decl.kind {
                    DeclKind::Module(inner) => {
                        ns = inner;
                    }
                    DeclKind::LayeredModules(stack) => {
                        let next = fq_ident.path.get(index + 1).unwrap_or(&fq_ident.name);
                        let mut found = false;
                        for n in stack.iter().rev() {
                            if n.names.contains_key(next) {
                                ns = n;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return None;
                        }
                    }
                    _ => return None,
                }
            } else {
                return None;
            }
        }

        ns.names.get(&fq_ident.name)
    }

    pub fn lookup(&self, ident: &Ident) -> HashSet<Ident> {
        fn lookup_in(module: &Module, ident: Ident) -> HashSet<Ident> {
            let (prefix, ident) = ident.pop_front();

            if let Some(ident) = ident {
                if let Some(entry) = module.names.get(&prefix) {
                    let redirected = match &entry.kind {
                        DeclKind::Module(ns) => ns.lookup(&ident),
                        DeclKind::LayeredModules(stack) => {
                            let mut r = HashSet::new();
                            for ns in stack.iter().rev() {
                                r = ns.lookup(&ident);

                                if !r.is_empty() {
                                    break;
                                }
                            }
                            r
                        }
                        _ => HashSet::new(),
                    };

                    return redirected
                        .into_iter()
                        .map(|i| Ident::from_name(&prefix) + i)
                        .collect();
                }
            } else if let Some(decl) = module.names.get(&prefix) {
                if let DeclKind::Module(inner) = &decl.kind {
                    if inner.names.contains_key(NS_SELF) {
                        return HashSet::from([Ident::from_path(vec![
                            prefix,
                            NS_SELF.to_string(),
                        ])]);
                    }
                }

                return HashSet::from([Ident::from_name(prefix)]);
            }
            HashSet::new()
        }

        log::trace!("lookup: {ident}");

        let mut res = HashSet::new();

        res.extend(lookup_in(self, ident.clone()));

        for redirect in &self.redirects {
            log::trace!("... following redirect {redirect}");
            let r = lookup_in(self, redirect.clone() + ident.clone());
            log::trace!("... result of redirect {redirect}: {r:?}");
            res.extend(r);
        }
        res
    }

    pub(super) fn insert_frame(&mut self, frame: &Lineage, namespace: &str) {
        let namespace = self.names.entry(namespace.to_string()).or_default();
        let namespace = namespace.kind.as_module_mut().unwrap();

        for (col_index, column) in frame.columns.iter().enumerate() {
            // determine input name
            let input_name = match column {
                LineageColumn::All { input_name, .. } => Some(input_name),
                LineageColumn::Single { name, .. } => name.as_ref().and_then(|n| n.path.first()),
            };

            // get or create input namespace
            let ns;
            if let Some(input_name) = input_name {
                let entry = match namespace.names.get_mut(input_name) {
                    Some(x) => x,
                    None => {
                        namespace.redirects.push(Ident::from_name(input_name));

                        let input = frame.find_input(input_name).unwrap();
                        let mut sub_ns = Module::default();

                        let self_decl = Decl {
                            declared_at: Some(input.id),
                            kind: DeclKind::InstanceOf(input.table.clone()),
                            ..Default::default()
                        };
                        sub_ns.names.insert(NS_SELF.to_string(), self_decl);

                        let sub_ns = Decl {
                            declared_at: Some(input.id),
                            kind: DeclKind::Module(sub_ns),
                            ..Default::default()
                        };

                        namespace.names.entry(input_name.clone()).or_insert(sub_ns)
                    }
                };
                ns = entry.kind.as_module_mut().unwrap()
            } else {
                ns = namespace;
            }

            // insert column decl
            match column {
                LineageColumn::All { input_name, .. } => {
                    let input = frame.inputs.iter().find(|i| &i.name == input_name).unwrap();

                    let kind = DeclKind::Infer(Box::new(DeclKind::Column(input.id)));
                    let declared_at = Some(input.id);
                    let decl = Decl {
                        kind,
                        declared_at,
                        order: col_index + 1,
                        ..Default::default()
                    };
                    ns.names.insert(NS_INFER.to_string(), decl);
                }
                LineageColumn::Single {
                    name: Some(name),
                    target_id,
                    ..
                } => {
                    let decl = Decl {
                        kind: DeclKind::Column(*target_id),
                        declared_at: None,
                        order: col_index + 1,
                        ..Default::default()
                    };
                    ns.names.insert(name.name.clone(), decl);
                }
                _ => {}
            }
        }
    }

    pub(super) fn insert_frame_col(&mut self, namespace: &str, name: String, id: usize) {
        let namespace = self.names.entry(namespace.to_string()).or_default();
        let namespace = namespace.kind.as_module_mut().unwrap();

        namespace.names.insert(name, DeclKind::Column(id).into());
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

    pub fn stack_push(&mut self, ident: &str, namespace: Module) {
        let entry = self
            .names
            .entry(ident.to_string())
            .or_insert_with(|| DeclKind::LayeredModules(Vec::new()).into());
        let stack = entry.kind.as_layered_modules_mut().unwrap();

        stack.push(namespace);
    }

    pub fn stack_pop(&mut self, ident: &str) -> Option<Module> {
        (self.names.get_mut(ident))
            .and_then(|e| e.kind.as_layered_modules_mut())
            .and_then(|stack| stack.pop())
    }

    pub(crate) fn into_exprs(self) -> HashMap<String, Expr> {
        self.names
            .into_iter()
            .map(|(k, v)| (k, *v.kind.into_expr().unwrap()))
            .collect()
    }

    pub(crate) fn from_exprs(exprs: HashMap<String, Expr>) -> Module {
        Module {
            names: exprs
                .into_iter()
                .map(|(key, expr)| {
                    let decl = Decl {
                        kind: DeclKind::Expr(Box::new(expr)),
                        ..Default::default()
                    };
                    (key, decl)
                })
                .collect(),
            ..Default::default()
        }
    }

    pub fn as_decls(&self) -> Vec<(Ident, &Decl)> {
        let mut r = Vec::new();
        for (name, decl) in &self.names {
            match &decl.kind {
                DeclKind::Module(module) => r.extend(
                    module
                        .as_decls()
                        .into_iter()
                        .map(|(inner, decl)| (Ident::from_name(name) + inner, decl)),
                ),
                _ => r.push((Ident::from_name(name), decl)),
            }
        }
        r
    }

    /// Recursively finds all declarations that end in suffix.
    pub fn find_by_suffix(&self, suffix: &str) -> Vec<Ident> {
        let mut res = Vec::new();

        for (name, decl) in &self.names {
            if let DeclKind::Module(module) = &decl.kind {
                let nested = module.find_by_suffix(suffix);
                res.extend(nested.into_iter().map(|x| x.prepend(vec![name.clone()])));
                continue;
            }

            if name == suffix {
                res.push(Ident::from_name(name));
            }
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::pl::{Expr, ExprKind, Literal};

    // TODO: tests / docstrings for `stack_pop` & `stack_push` & `insert_frame`
    #[test]
    fn test_module() {
        let mut module = Module::default();

        let ident = Ident::from_name("test_name");
        let expr: Expr = Expr::new(ExprKind::Literal(Literal::Integer(42)));
        let decl: Decl = DeclKind::Expr(Box::new(expr)).into();

        assert!(module.insert(ident.clone(), decl.clone()).is_ok());
        assert_eq!(module.get(&ident).unwrap(), &decl);
        assert_eq!(module.get_mut(&ident).unwrap(), &decl);

        // Lookup
        let lookup_result = module.lookup(&ident);
        assert_eq!(lookup_result.len(), 1);
        assert!(lookup_result.contains(&ident));
    }

    #[test]
    fn test_module_shadow_unshadow() {
        let mut module = Module::default();

        let ident = Ident::from_name("test_name");
        let expr: Expr = Expr::new(ExprKind::Literal(Literal::Integer(42)));
        let decl: Decl = DeclKind::Expr(Box::new(expr)).into();

        module.insert(ident.clone(), decl.clone()).unwrap();

        module.shadow("test_name");
        assert!(module.get(&ident) != Some(&decl));

        module.unshadow("test_name");
        assert_eq!(module.get(&ident).unwrap(), &decl);
    }
}
