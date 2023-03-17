//! Semantic resolver (name resolution, type checking and lowering to RQ)

mod context;
mod lowering;
mod module;
pub mod reporting;
mod resolver;
mod static_analysis;
mod transforms;
mod type_resolver;

pub use self::context::Context;
pub use self::module::Module;

use crate::ast::pl::frame::{Frame, FrameColumn};
use crate::ast::pl::Stmt;
use crate::ast::rq::Query;
use crate::PRQL_VERSION;

use anyhow::{bail, Result};
use semver::{Version, VersionReq};

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve(statements: Vec<Stmt>) -> Result<Query> {
    let context = load_std_lib();

    let (statements, context) = resolver::resolve(statements, context)?;

    let query = lowering::lower_ast_to_ir(statements, context)?;

    if let Some(ref version) = query.def.version {
        check_query_version(version, &PRQL_VERSION)?;
    }

    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve_only(
    statements: Vec<Stmt>,
    context: Option<Context>,
) -> Result<(Vec<Stmt>, Context)> {
    let context = context.unwrap_or_else(load_std_lib);

    resolver::resolve(statements, context)
}

pub fn load_std_lib() -> Context {
    use crate::parser::parse;
    let std_lib = include_str!("./std.prql");
    let statements = parse(std_lib).unwrap();

    let context = Context {
        root_mod: Module::new(),
        ..Context::default()
    };

    let (_, context) = resolver::resolve(statements, context).unwrap();
    context
}

fn check_query_version(query_version: &VersionReq, prql_version: &Version) -> Result<()> {
    if !query_version.matches(prql_version) {
        bail!("This query uses a version of PRQL that is not supported by your prql-compiler. You may want to upgrade the compiler.");
    }

    Ok(())
}

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
                ExternRef: employees
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
