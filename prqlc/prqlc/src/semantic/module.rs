use std::collections::{HashMap, HashSet};

use super::{
    NS_DEFAULT_DB, NS_INFER, NS_INFER_MODULE, NS_MAIN, NS_PARAM, NS_QUERY_DEF, NS_SELF,
    NS_SHADOWING_COL, NS_STD, NS_THAT, NS_THIS,
};
use crate::ir::decl::{Decl, DeclKind, Module, RootModule, TableDecl, TableExpr};
use crate::ir::pl::{Annotation, Expr, Ident, Lineage, LineageColumn};
use crate::pr::QueryDef;
use crate::pr::{Span, Ty, TyKind, TyTupleField};
use crate::Error;
use crate::Result;

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
            ],
        }
    }

    pub fn new_database() -> Module {
        let names = HashMap::from([
            (
                NS_INFER.to_string(),
                Decl::from(DeclKind::Infer(Box::new(DeclKind::TableDecl(TableDecl {
                    ty: Some(Ty::relation(vec![TyTupleField::Wildcard(None)])),
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
            let decl = ns.names.get(part)?;

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
        }

        ns.names.get(&fq_ident.name)
    }

    /// Recursively list all idents within this module. Useful for debugging.
    pub fn all_names(&self, prefix: Option<&Ident>) -> Vec<Ident> {
        let mut rv = Vec::new();

        for (name, decl) in &self.names {
            let name = match prefix {
                Some(p) => p.clone() + Ident::from_name(name),
                None => Ident::from_name(name),
            };
            rv.push(name.clone());

            match &decl.kind {
                DeclKind::Module(inner) => {
                    rv.extend(inner.all_names(Some(&name)));
                }
                DeclKind::LayeredModules(stack) => {
                    for inner in stack {
                        rv.extend(inner.all_names(Some(&name)));
                    }
                }
                _ => {}
            }
        }

        rv
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
                    // A column shadowing this relation's name (nested under
                    // `NS_SHADOWING_COL`) takes precedence for leaf access. We
                    // return the bare relation ident; the ident resolver then
                    // unwraps it to the nested column (keeping the emitted name
                    // clean). Qualified `prefix.col` access still recurses into
                    // the relation below. Gate on it being a `Column` to match
                    // the resolver's unwrap site, keeping the invariant explicit.
                    if matches!(
                        inner.names.get(NS_SHADOWING_COL).map(|d| &d.kind),
                        Some(DeclKind::Column(_))
                    ) {
                        return HashSet::from([Ident::from_name(prefix)]);
                    }
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
            if !r.is_empty() {
                res.remove(ident);
                res.extend(r);
            }
        }
        res
    }

    pub(super) fn insert_frame(&mut self, lineage: &Lineage, namespace: &str) {
        let namespace = self.names.entry(namespace.to_string()).or_default();
        let namespace = namespace.kind.as_module_mut().unwrap();

        let lin_ty = *ty_of_lineage(lineage).kind.into_array().unwrap().unwrap();

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

                        log::trace!("find_input_by_name {input_name} {:#?}", lineage.inputs);
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
                            .or(Some(Ty::new(TyKind::Tuple(vec![TyTupleField::Wildcard(
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
                    // Input might not exist if lineage references an outer scope
                    // (e.g., join inside group). This is an error caught during
                    // lowering - skip here to avoid panic during resolution.
                    if let Some(input) = lineage.find_input(*input_id) {
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
                    insert_possibly_shadowing_col(ns, &name.name, decl);
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

        insert_possibly_shadowing_col(namespace, &name, DeclKind::Column(id).into());
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
            return Err(Error::new_simple(format!(
                "duplicate declarations of {ident}"
            )));
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
}

/// Insert a column into `namespace`. If the column's name collides with a
/// source input namespace (e.g. `from bar | derive {bar = ...}`), nest it
/// inside that namespace under [`NS_SHADOWING_COL`] rather than overwriting it.
/// The input namespace holds the `NS_INFER` template and is the redirect target
/// used to infer every other column, so clobbering it would break resolution of
/// all sibling columns, and removing it would break qualified `bar.col` access.
/// Leaf access to `bar` resolves to the nested shadowing column (see
/// [`Module::lookup`] and the ident resolver).
fn insert_possibly_shadowing_col(namespace: &mut Module, name: &str, decl: Decl) {
    if let Some(Decl {
        kind: DeclKind::Module(input),
        ..
    }) = namespace.names.get_mut(name)
    {
        // Last writer wins on a repeated shadow. This is safe because each
        // pipeline step rebuilds the frame from lineage, so a single namespace
        // never accumulates two shadows of the same name.
        input.names.insert(NS_SHADOWING_COL.to_string(), decl);
    } else {
        namespace.names.insert(name.to_string(), decl);
    }
}

pub fn ty_of_lineage(lineage: &Lineage) -> Ty {
    Ty::relation(
        lineage
            .columns
            .iter()
            .map(|col| match col {
                LineageColumn::All { .. } => TyTupleField::Wildcard(None),
                LineageColumn::Single { name, .. } => {
                    TyTupleField::Single(name.as_ref().map(|i| i.name.clone()), None)
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use prqlc_parser::lexer::lr::Literal;

    use super::*;
    use crate::ir::pl::ExprKind;

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

    // A column that shares its name with the source relation used to clobber
    // the relation's input namespace, breaking inference of every other
    // column. See the `insert_frame` collision handling above.
    #[test]
    fn test_column_shadows_relation_name() {
        use insta::assert_snapshot;

        // minimal repro: derived column `bar` shadows source `bar`
        assert_snapshot!(crate::tests::compile(
            "from bar | derive { bar = this.a } | select { this.x, this.bar }"
        ).unwrap(), @"
        SELECT
          x,
          a AS bar
        FROM
          bar
        ");

        // original source columns are still inferable after the collision
        assert_snapshot!(crate::tests::compile(
            "from bar | derive { bar = this.a } | select { this.a }"
        ).unwrap(), @r"
        SELECT
          a
        FROM
          bar
        ");

        // collision against a relation alias (not the table name)
        assert_snapshot!(crate::tests::compile(
            "from b=bar | derive { b = this.a } | select { this.x, this.b }"
        ).unwrap(), @"
        SELECT
          x,
          a AS b
        FROM
          bar AS b
        ");

        // full real-world shape: group/aggregate/sort then a column named
        // after the source relation
        assert_snapshot!(crate::tests::compile(
            "from sales.sales \
             | group { this.category } ( aggregate { col1 = sum this.amount } ) \
             | sort { this.category } \
             | derive { sales = this.col1 } \
             | select { this.category, this.sales }"
        ).unwrap(), @"
        WITH table_0 AS (
          SELECT
            category,
            COALESCE(SUM(amount), 0) AS _expr_0
          FROM
            sales.sales
          GROUP BY
            category
        )
        SELECT
          category,
          _expr_0 AS sales
        FROM
          table_0
        ORDER BY
          category
        ");
    }

    // References to *other* columns of the shadowed relation within the *same*
    // tuple must still resolve: nesting the shadowing column inside the input
    // namespace (rather than overwriting it) keeps the input's inference
    // template available for siblings resolved later in the tuple.
    #[test]
    fn test_column_shadows_relation_name_intra_tuple() {
        use insta::assert_snapshot;

        // `this.x` should still resolve after `bar` shadows the source `bar`
        // within the same `derive`.
        assert_snapshot!(crate::tests::compile(
            "from bar | derive { bar = this.a, x2 = this.x }"
        ).unwrap(), @r"
        SELECT
          *,
          a AS bar,
          x AS x2
        FROM
          bar
        ");
    }

    // Explicit qualified access to a *different* column of the shadowed input
    // (e.g. `bar.x`) keeps resolving to the relation, while the bare name
    // (`this.bar`) resolves to the shadowing column. Both can appear in the
    // same `select` without one being dropped during SQL projection.
    #[test]
    fn test_column_shadows_relation_name_qualified_access() {
        use insta::assert_snapshot;

        assert_snapshot!(crate::tests::compile(
            "from bar | join foo (==id) | derive { bar = bar.a } | select { bar.x, this.bar }"
        ).unwrap(), @"
        SELECT
          bar.x,
          bar.a AS bar
        FROM
          bar
          INNER JOIN foo ON bar.id = foo.id
        ");
    }

    // Wildcard expansion over a shadowed relation must not crash: `this.*` and
    // `bar.*` need the relation *module*, which is preserved alongside the
    // nested shadowing column.
    #[test]
    fn test_column_shadows_relation_name_wildcard() {
        use insta::assert_snapshot;

        assert_snapshot!(crate::tests::compile(
            "from bar | derive { bar = this.a } | select this.*"
        ).unwrap(), @"
        SELECT
          *
        FROM
          bar
        ");

        assert_snapshot!(crate::tests::compile(
            "from bar | derive { bar = this.a } | select bar.*"
        ).unwrap(), @"
        SELECT
          *
        FROM
          bar
        ");
    }
}
