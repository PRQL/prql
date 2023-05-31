//! Semantic resolver (name resolution, type checking and lowering to RQ)

mod context;
mod lowering;
mod module;
pub mod reporting;
mod resolver;
mod static_analysis;
mod transforms;
mod type_resolver;

use anyhow::Result;
use itertools::Itertools;
use std::path::{Path, PathBuf};

pub use self::context::Context;
pub use self::module::Module;
pub use lowering::lower_to_ir;

use crate::ast::pl::{Lineage, LineageColumn, Stmt};
use crate::ast::rq::Query;
use crate::error::WithErrorInfo;
use crate::{Error, SourceTree};

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_and_lower_single(statements: Vec<Stmt>) -> Result<Query> {
    let context = load_std_lib();

    let context = resolver::resolve(statements, vec![], context)?;

    let (query, _) = lowering::lower_to_ir(context, &[])?;

    Ok(query)
}

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_and_lower(file_tree: SourceTree<Vec<Stmt>>, main_path: &[String]) -> Result<Query> {
    let context = resolve(file_tree, None)?;

    let (query, _) = lowering::lower_to_ir(context, main_path)?;
    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve_single(statements: Vec<Stmt>, context: Option<Context>) -> Result<Context> {
    let tree = SourceTree::single(PathBuf::from(""), statements);

    resolve(tree, context)
}

/// Runs semantic analysis on the query.
pub fn resolve(file_tree: SourceTree<Vec<Stmt>>, context: Option<Context>) -> Result<Context> {
    let mut context = context.unwrap_or_else(load_std_lib);
    for (path, stmts) in normalize(file_tree)? {
        context = resolver::resolve(stmts, path, context)?;
    }
    Ok(context)
}

pub fn load_std_lib() -> Context {
    let std_lib = SourceTree::from(include_str!("./std.prql"));
    let statements = crate::parser::parse_tree(&std_lib).unwrap();
    let statements = statements.sources.into_values().next().unwrap();

    let context = Context {
        root_mod: Module::new_root(),
        ..Context::default()
    };

    resolver::resolve(statements, vec![NS_STD.to_string()], context).unwrap()
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

fn normalize(mut tree: SourceTree<Vec<Stmt>>) -> Result<Vec<(Vec<String>, Vec<Stmt>)>> {
    // find root
    let root_path = PathBuf::from("");

    if tree.sources.get(&root_path).is_none() {
        if tree.sources.len() == 1 {
            // if there is only one file, use that as the root
            let (_, only) = tree.sources.drain().exactly_one().unwrap();
            tree.sources.insert(root_path, only);
        } else if let Some(under) = tree.sources.keys().find(|p| path_starts_with(p, "_")) {
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
            .with_help("add a file prefixed with `_` to the root directory")
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

fn path_starts_with(p: &Path, prefix: &str) -> bool {
    p.components()
        .next()
        .and_then(|x| x.as_os_str().to_str())
        .map_or(false, |x| x.starts_with(prefix))
}

pub const NS_STD: &str = "std";
pub const NS_FRAME: &str = "_frame";
pub const NS_FRAME_RIGHT: &str = "_right";
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_yaml_snapshot;

    use super::resolve_and_lower_single;
    use crate::{ast::rq::Query, parser::parse_single};

    fn parse_and_resolve(query: &str) -> Result<Query> {
        resolve_and_lower_single(parse_single(query)?)
    }

    #[test]
    fn test_resolve_01() {
        assert_yaml_snapshot!(parse_and_resolve(r###"
        from employees
        select !{foo}
        "###).unwrap().relation.columns, @r###"
        ---
        - Wildcard
        "###)
    }

    #[test]
    fn test_resolve_02() {
        assert_yaml_snapshot!(parse_and_resolve(r###"
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
        assert_yaml_snapshot!(parse_and_resolve(r###"
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
        assert_yaml_snapshot!(parse_and_resolve(r###"
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
        assert_yaml_snapshot!(parse_and_resolve(r###"
        prql target:sql.mssql version:"0"

        from employees
        "###).unwrap(), @r###"
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

        assert!(parse_and_resolve(
            r###"
        prql target:sql.bigquery version:foo
        from employees
        "###,
        )
        .is_err());

        assert!(parse_and_resolve(
            r###"
        prql target:sql.bigquery version:"25"
        from employees
        "###,
        )
        .is_err());

        assert!(parse_and_resolve(
            r###"
        prql target:sql.yah version:foo
        from employees
        "###,
        )
        .is_err());
    }

    #[test]
    fn check_valid_version() {
        let stmt = format!(
            r#"
        prql version:"{}"
        "#,
            env!("CARGO_PKG_VERSION_MAJOR")
        );
        assert!(parse_single(&stmt).is_ok());

        let stmt = format!(
            r#"
            prql version:"{}.{}"
            "#,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR")
        );
        assert!(parse_single(&stmt).is_ok());

        let stmt = format!(
            r#"
            prql version:"{}.{}.{}"
            "#,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        );
        assert!(parse_single(&stmt).is_ok());
    }

    #[test]
    fn check_invalid_version() {
        let stmt = format!(
            "prql version:{}\n",
            env!("CARGO_PKG_VERSION_MAJOR").parse::<usize>().unwrap() + 1
        );
        assert!(parse_single(&stmt).is_err());
    }
}
