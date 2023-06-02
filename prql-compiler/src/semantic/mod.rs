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
use std::path::PathBuf;

pub use self::context::Context;
pub use self::module::Module;
use self::resolver::Resolver;
pub use self::resolver::ResolverOptions;
pub use lowering::lower_to_ir;

use crate::ast::pl::{Lineage, LineageColumn, Stmt};
use crate::ast::rq::Query;
use crate::error::WithErrorInfo;
use crate::{Error, SourceTree};

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_and_lower(file_tree: SourceTree<Vec<Stmt>>, main_path: &[String]) -> Result<Query> {
    let context = resolve(file_tree, Default::default())?;

    let (query, _) = lowering::lower_to_ir(context, main_path)?;
    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve(mut file_tree: SourceTree<Vec<Stmt>>, options: ResolverOptions) -> Result<Context> {
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
    let context = Context {
        root_mod: Module::new_root(),
        ..Context::default()
    };
    let mut resolver = Resolver::new(context, options);

    // resolve sources one by one
    // TODO: recursive references
    for (path, stmts) in normalize(file_tree)? {
        resolver.current_module_path = path;
        resolver.fold_statements(stmts)?;
    }

    Ok(resolver.context)
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

fn normalize(mut tree: SourceTree<Vec<Stmt>>) -> Result<Vec<(Vec<String>, Vec<Stmt>)>> {
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
pub mod test {
    use anyhow::Result;
    use insta::assert_yaml_snapshot;

    use crate::ast::rq::Query;
    use crate::parser::{parse, parse_single};

    use super::{resolve, resolve_and_lower, Context};

    pub fn parse_resolve_and_lower(query: &str) -> Result<Query> {
        let mut source_tree = query.into();
        super::load_std_lib(&mut source_tree);

        resolve_and_lower(parse(&source_tree)?, &[])
    }

    pub fn parse_and_resolve(query: &str) -> Result<Context> {
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
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
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

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.bigquery version:foo
        from employees
        "###,
        )
        .is_err());

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.bigquery version:"25"
        from employees
        "###,
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
