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

use crate::ast::pl::{Frame, FrameColumn, Stmt};
use crate::ast::rq::Query;
use crate::FileTree;

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve(statements: Vec<Stmt>) -> Result<Query> {
    let context = load_std_lib();

    let context = resolver::resolve(statements, vec![], context)?;

    let query = lowering::lower_to_ir(context, &[])?;

    Ok(query)
}

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_tree(file_tree: FileTree<Vec<Stmt>>, main_path: Vec<String>) -> Result<Query> {
    let mut context = load_std_lib();

    for (path, stmts) in file_tree.files {
        let path = os_path_to_prql_path(path)?;

        context = resolver::resolve(stmts, path, context)?;
    }

    let query = lowering::lower_to_ir(context, &main_path)?;

    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve_only(statements: Vec<Stmt>, context: Option<Context>) -> Result<Context> {
    let context = context.unwrap_or_else(load_std_lib);

    resolver::resolve(statements, vec![], context)
}

pub fn load_std_lib() -> Context {
    use crate::parser::parse;
    let std_lib = include_str!("./std.prql");
    let statements = parse(std_lib).unwrap();

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

    use super::resolve;
    use crate::{ast::rq::Query, parser::parse};

    fn parse_and_resolve(query: &str) -> Result<Query> {
        resolve(parse(query)?)
    }

    #[test]
    fn test_resolve_01() {
        assert_yaml_snapshot!(parse_and_resolve(r###"
        from employees
        select ![foo]
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
            derive [next_four_days = sum b]
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
        select [a.*]
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
        select [a, a, a = a + 1]
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
        assert!(parse(&stmt).is_ok());

        let stmt = format!(
            r#"
            prql version:"{}.{}"
            "#,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR")
        );
        assert!(parse(&stmt).is_ok());

        let stmt = format!(
            r#"
            prql version:"{}.{}.{}"
            "#,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        );
        assert!(parse(&stmt).is_ok());
    }

    #[test]
    fn check_invalid_version() {
        let stmt = format!(
            "prql version:{}\n",
            env!("CARGO_PKG_VERSION_MAJOR").parse::<usize>().unwrap() + 1
        );
        assert!(parse(&stmt).is_err());
    }
}
