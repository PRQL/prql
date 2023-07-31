//! Semantic resolver (name resolution, type checking and lowering to RQ)

mod ast_expand;
mod decl;
mod eval;
mod lowering;
mod module;
pub mod reporting;
mod resolver;
mod root_module;
mod static_analysis;

use anyhow::Result;
use itertools::Itertools;
use std::path::PathBuf;

pub use self::module::Module;
use self::resolver::Resolver;
pub use self::resolver::ResolverOptions;
pub use self::root_module::RootModule;
pub use eval::eval;
pub use lowering::lower_to_ir;

use crate::ir::pl::{self, Lineage, LineageColumn, ModuleDef, Stmt, StmtKind, TypeDef, VarDef};
use crate::ir::rq::RelationalQuery;
use crate::WithErrorInfo;
use crate::{Error, Reason, SourceTree};

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_and_lower(
    file_tree: SourceTree<Vec<prqlc_ast::stmt::Stmt>>,
    main_path: &[String],
) -> Result<RelationalQuery> {
    let context = resolve(file_tree, Default::default())?;

    let (query, _) = lowering::lower_to_ir(context, main_path)?;
    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve(
    mut file_tree: SourceTree<Vec<prqlc_ast::stmt::Stmt>>,
    options: ResolverOptions,
) -> Result<RootModule> {
    // inject std module if it does not exist
    if !file_tree.sources.contains_key(&PathBuf::from("std.prql")) {
        let mut source_tree = SourceTree {
            sources: Default::default(),
            source_ids: file_tree.source_ids.clone(),
        };
        load_std_lib(&mut source_tree);
        let ast = crate::parser::parse(&source_tree).unwrap();
        let (path, content) = ast.sources.into_iter().next().unwrap();
        file_tree.insert(path, content);
    }

    // init empty context
    let mut root_module = RootModule::new();
    let mut resolver = Resolver::new(&mut root_module, options);

    // resolve sources one by one
    // TODO: recursive references
    for (path, stmts) in normalize(file_tree)? {
        let stmts = ast_expand::expand_stmts(stmts)?;

        resolver.resolve(path, stmts)?;
    }

    Ok(root_module)
}

/// Preferred way of injecting std module.
pub fn load_std_lib(source_tree: &mut SourceTree) {
    let path = PathBuf::from("std.prql");
    let content = include_str!("./std.prql");

    source_tree.insert(path, content.to_string());
}

pub fn os_path_to_prql_path(path: PathBuf) -> Result<Vec<String>> {
    // remove file format extension
    let path = path.with_extension("");

    // split by /
    path.components()
        .map(|x| {
            x.as_os_str()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid file path: {path:?}"))
                .map(str::to_string)
        })
        .try_collect()
}

fn normalize(
    mut tree: SourceTree<Vec<prqlc_ast::stmt::Stmt>>,
) -> Result<Vec<(Vec<String>, Vec<prqlc_ast::stmt::Stmt>)>> {
    // find root
    let root_path = PathBuf::from("");

    if tree.sources.get(&root_path).is_none() {
        if tree.sources.len() == 1 {
            // if there is only one file, use that as the root
            let (_, only) = tree.sources.drain().exactly_one().unwrap();
            tree.sources.insert(root_path, only);
        } else if let Some(under) = tree.sources.keys().find(path_starts_with_uppercase) {
            // if there is a path that starts with `_`, that's the root
            let under = tree.sources.remove(&under.clone()).unwrap();
            tree.sources.insert(root_path, under);
        } else {
            let file_names = tree
                .sources
                .keys()
                .map(|p| format!(" - {}", p.to_str().unwrap_or_default()))
                .sorted()
                .join("\n");

            return Err(Error::new_simple(format!(
                "Cannot find the root module within the following files:\n{file_names}"
            ))
            .push_hint("add a file prefixed with `_` to the root directory")
            .with_code("E0002")
            .into());
        }
    }

    // TODO: make sure that the module tree is normalized

    // TODO: find correct resolution order

    let mut modules = Vec::with_capacity(tree.sources.len());
    for (path, stmts) in tree.sources {
        let path = os_path_to_prql_path(path)?;
        modules.push((path, stmts));
    }
    modules.sort_unstable_by_key(|(path, _)| path.join("."));
    modules.reverse();

    Ok(modules)
}

fn path_starts_with_uppercase(p: &&PathBuf) -> bool {
    p.components()
        .next()
        .and_then(|x| x.as_os_str().to_str())
        .and_then(|x| x.chars().next())
        .map_or(false, |x| x.is_uppercase())
}

pub const NS_STD: &str = "std";
pub const NS_THIS: &str = "this";
pub const NS_THAT: &str = "that";
pub const NS_PARAM: &str = "_param";
pub const NS_DEFAULT_DB: &str = "default_db";
pub const NS_QUERY_DEF: &str = "prql";
pub const NS_MAIN: &str = "main";

// refers to the containing module (direct parent)
pub const NS_SELF: &str = "_self";

// implies we can infer new non-module declarations in the containing module
pub const NS_INFER: &str = "_infer";

// implies we can infer new module declarations in the containing module
pub const NS_INFER_MODULE: &str = "_infer_module";

impl Stmt {
    pub(crate) fn name(&self) -> &str {
        match &self.kind {
            StmtKind::QueryDef(_) => NS_QUERY_DEF,
            StmtKind::VarDef(VarDef { name, .. }) => name,
            StmtKind::TypeDef(TypeDef { name, .. }) => name,
            StmtKind::ModuleDef(ModuleDef { name, .. }) => name,
        }
    }
}

impl pl::Expr {
    fn try_cast<T, F, S2: ToString>(self, f: F, who: Option<&str>, expected: S2) -> Result<T, Error>
    where
        F: FnOnce(pl::ExprKind) -> Result<T, pl::ExprKind>,
    {
        f(self.kind).map_err(|i| {
            Error::new(Reason::Expected {
                who: who.map(|s| s.to_string()),
                expected: expected.to_string(),
                found: format!("`{}`", write_pl(pl::Expr::new(i))),
            })
            .with_span(self.span)
        })
    }
}

/// Write a PL IR to string.
///
/// Because PL needs to be restricted back to AST, ownerships of expr is required.
pub fn write_pl(expr: pl::Expr) -> String {
    let expr = ast_expand::restrict_expr(expr);

    crate::codegen::write_expr(&expr)
}
#[cfg(test)]
pub mod test {
    use anyhow::Result;
    use insta::assert_yaml_snapshot;

    use crate::ir::rq::RelationalQuery;
    use crate::parser::parse;

    use super::{resolve, resolve_and_lower, RootModule};

    pub fn parse_resolve_and_lower(query: &str) -> Result<RelationalQuery> {
        let mut source_tree = query.into();
        super::load_std_lib(&mut source_tree);

        resolve_and_lower(parse(&source_tree)?, &[])
    }

    pub fn parse_and_resolve(query: &str) -> Result<RootModule> {
        let mut source_tree = query.into();
        super::load_std_lib(&mut source_tree);

        resolve(parse(&source_tree)?, Default::default())
    }

    #[test]
    fn test_resolve_01() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from employees
        select !{foo}
        "###).unwrap().relation.columns, @r###"
        ---
        - Wildcard
        "###)
    }

    #[test]
    fn test_resolve_02() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from foo
        sort day
        window range:-4..4 (
            derive {next_four_days = sum b}
        )
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: day
        - Single: b
        - Wildcard
        - Single: next_four_days
        "###)
    }

    #[test]
    fn test_resolve_03() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from a=albums
        filter is_sponsored
        select {a.*}
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: is_sponsored
        - Wildcard
        "###)
    }

    #[test]
    fn test_resolve_04() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from x
        select {a, a, a = a + 1}
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: ~
        - Single: ~
        - Single: a
        "###)
    }

    #[test]
    fn test_header() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r#"
        prql target:sql.mssql version:"0"

        from employees
        "#).unwrap(), @r###"
        ---
        def:
          version: ^0
          other:
            target: sql.mssql
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  - employees
              columns:
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Wildcard
                      - 0
                  name: employees
              - Select:
                  - 0
          columns:
            - Wildcard
        "### );

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.bigquery version:foo
        from employees
        "###,
        )
        .is_err());

        assert!(parse_resolve_and_lower(
            r#"
        prql target:sql.bigquery version:"25"
        from employees
        "#,
        )
        .is_err());

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.yah version:foo
        from employees
        "###,
        )
        .is_err());
    }
}
