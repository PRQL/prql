use std::collections::{HashMap, HashSet};

use anyhow::Result;
use prqlc_ast::stmt::QueryDef;
use prqlc_ast::{Literal, Span, TupleField, Ty, TyKind};

use crate::ir::pl::{Annotation, Expr, Ident, Lineage, LineageColumn};
use crate::Error;

use super::{
    NS_DEFAULT_DB, NS_GENERIC, NS_INFER, NS_INFER_MODULE, NS_MAIN, NS_PARAM, NS_QUERY_DEF, NS_SELF,
    NS_STD, NS_THAT, NS_THIS,
};
use crate::ir::decl::{Decl, DeclKind, Module, RootModule, TableDecl, TableExpr};

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
            redirects: vec![
                Ident::from_name(NS_THIS),
                Ident::from_name(NS_THAT),
                Ident::from_name(NS_PARAM),
                Ident::from_name(NS_STD),
                Ident::from_name(NS_GENERIC),
            ],
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

    pub(super) fn insert_frame(&mut self, lineage: &Lineage, namespace: &str) {
        let namespace = self.names.entry(namespace.to_string()).or_default();
        let namespace = namespace.kind.as_module_mut().unwrap();

        let lin_ty = *ty_of_lineage(lineage).kind.into_array().unwrap();

        for (col_index, column) in lineage.columns.iter().enumerate() {
            // determine input name
            let input_name = match column {
                LineageColumn::All { input_id, .. } => {
                    lineage.find_input(*input_id).map(|i| &i.name)
                }
                LineageColumn::Single { name, .. } => name.as_ref().and_then(|n| n.path.first()),
            };

            // get or create input namespace
            let ns;
            if let Some(input_name) = input_name {
                let entry = match namespace.names.get_mut(input_name) {
                    Some(x) => x,
                    None => {
                        namespace.redirects.push(Ident::from_name(input_name));

                        let input = lineage.find_input_by_name(input_name).unwrap();
                        let order = lineage.inputs.iter().position(|i| i.id == input.id);
                        let order = order.unwrap();

                        let mut sub_ns = Module::default();

                        let self_ty = lin_ty.clone().kind.into_tuple().unwrap();
                        let self_ty = self_ty
                            .into_iter()
                            .flat_map(|x| x.into_single())
                            .find(|(name, _)| name.as_ref() == Some(input_name))
                            .and_then(|(_, ty)| ty)
                            .or(Some(Ty::new(TyKind::Tuple(vec![TupleField::Wildcard(
                                None,
                            )]))));

                        let self_decl = Decl {
                            declared_at: Some(input.id),
                            kind: DeclKind::InstanceOf(input.table.clone(), self_ty),
                            ..Default::default()
                        };
                        sub_ns.names.insert(NS_SELF.to_string(), self_decl);

                        let sub_ns = Decl {
                            declared_at: Some(input.id),
                            order,
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
                LineageColumn::All { input_id, .. } => {
                    let input = lineage.find_input(*input_id).unwrap();

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

        // insert namespace._self with correct type
        namespace.names.insert(
            NS_SELF.to_string(),
            Decl::from(DeclKind::InstanceOf(Ident::from_name(""), Some(lin_ty))),
        );
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

    /// Recursively finds all declarations with an annotation that has a specific name.
    pub fn find_by_annotation_name(&self, annotation_name: &Ident) -> Vec<Ident> {
        let mut res = Vec::new();

        for (name, decl) in &self.names {
            if let DeclKind::Module(module) = &decl.kind {
                let nested = module.find_by_annotation_name(annotation_name);
                res.extend(nested.into_iter().map(|x| x.prepend(vec![name.clone()])));
            }

            let has_annotation = decl_has_annotation(decl, annotation_name);
            if has_annotation {
                res.push(Ident::from_name(name));
            }
        }
        res
    }
}

fn decl_has_annotation(decl: &Decl, annotation_name: &Ident) -> bool {
    for ann in &decl.annotations {
        if super::is_ident_or_func_call(&ann.expr, annotation_name) {
            return true;
        }
    }
    false
}

type HintAndSpan = (Option<String>, Option<Span>);

impl RootModule {
    pub(super) fn declare(
        &mut self,
        ident: Ident,
        decl: DeclKind,
        id: Option<usize>,
        annotations: Vec<Annotation>,
    ) -> Result<()> {
        let existing = self.module.get(&ident);
        if existing.is_some() {
            return Err(Error::new_simple(format!("duplicate declarations of {ident}")).into());
        }

        let decl = Decl {
            kind: decl,
            declared_at: id,
            order: 0,
            annotations,
        };
        self.module.insert(ident, decl).unwrap();
        Ok(())
    }

    /// Finds that main pipeline given a path to either main itself or its parent module.
    /// Returns main expr and fq ident of the decl.
    pub fn find_main_rel(&self, path: &[String]) -> Result<(&TableExpr, Ident), HintAndSpan> {
        let (decl, ident) = self.find_main(path).map_err(|x| (x, None))?;

        let span = decl
            .declared_at
            .and_then(|id| self.span_map.get(&id))
            .cloned();

        let decl = (decl.kind.as_table_decl())
            .ok_or((Some(format!("{ident} is not a relational variable")), span))?;

        Ok((&decl.expr, ident))
    }

    pub fn find_main(&self, path: &[String]) -> Result<(&Decl, Ident), Option<String>> {
        let mut tried_idents = Vec::new();

        // is path referencing the relational var directly?
        if !path.is_empty() {
            let ident = Ident::from_path(path.to_vec());
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

            let ident = Ident::from_path(path);
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

    pub fn find_query_def(&self, main: &Ident) -> Option<&QueryDef> {
        let ident = Ident {
            path: main.path.clone(),
            name: NS_QUERY_DEF.to_string(),
        };

        let decl = self.module.get(&ident)?;
        decl.kind.as_query_def()
    }

    /// Finds all main pipelines.
    pub fn find_mains(&self) -> Vec<Ident> {
        self.module.find_by_suffix(NS_MAIN)
    }

    /// Finds declarations that are annotated with a specific name.
    pub fn find_by_annotation_name(&self, annotation_name: &Ident) -> Vec<Ident> {
        self.module.find_by_annotation_name(annotation_name)
    }
}

pub fn ty_of_lineage(lineage: &Lineage) -> Ty {
    Ty::relation(
        lineage
            .columns
            .iter()
            .map(|col| match col {
                LineageColumn::All { .. } => TupleField::Wildcard(None),
                LineageColumn::Single { name, .. } => TupleField::Single(
                    name.as_ref().map(|i| i.name.clone()),
                    Some(Ty::new(Literal::Null)),
                ),
            })
            .collect(),
    )
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
