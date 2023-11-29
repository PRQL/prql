#![cfg(not(target_family = "wasm"))]
use std::{env, fs};

use insta::{assert_snapshot, glob};

use prql_compiler::sql::Dialect;
use prql_compiler::{Options, Target};

#[test]
fn test_queries_compile() {
    env_logger::init();

    let target = Target::Sql(Some(Dialect::Generic));
    let options = Options::default().no_signature().with_target(target);

    // We're currently not testing for each dialect, as it's a lot of snapshots.
    // We can consider doing that if helpful.
    glob!("queries/**/*.prql", |path| {
        log::debug!("testing {path:?}");
        let prql = fs::read_to_string(path).unwrap();
        let sql = prql_compiler::compile(&prql, &options).unwrap();
        assert_snapshot!("sql", &sql, &prql)
    });
}

#[test]
fn test_queries_fmt() {
    glob!("queries/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();

        let pl = prql_compiler::prql_to_pl(&prql).unwrap();
        let formatted = prql_compiler::pl_to_prql(pl).unwrap();

        assert_snapshot!("fmt", &formatted, &prql)
    });
}

#[cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]
#[test]
fn test_queries_dbs() {
    use std::{env, fs};

    use anyhow::Context;
    use insta::{assert_snapshot, glob};
    use itertools::Itertools;
    use strum::IntoEnumIterator;

    use prql_compiler::Options;
    use prql_compiler::{sql::Dialect, sql::SupportLevel, Target::Sql};

    use crate::dbs::connection::DbConnection;
    use crate::dbs::IntegrationTest;

    let runtime = &*crate::dbs::RUNTIME;

    let mut connections: Vec<DbConnection> = Dialect::iter()
        .filter(|dialect| {
            matches!(
                dialect.support_level(),
                SupportLevel::Supported | SupportLevel::Unsupported
            )
        })
        // The filtering is not a great design, since it doesn't proactively
        // check that we can get connections; but it's a compromise given we
        // implement the external_dbs feature using this.
        .filter_map(|dialect| dialect.get_connection())
        .collect();

    for con in &mut connections {
        con.setup_connection(runtime);
    }

    // Each connection has a different data_file_root, so we need to replace.
    let re = regex::Regex::new("data_file_root").unwrap();

    // for each of the queries
    glob!("queries/**/*.prql", |path| {
        let test_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        let prql = fs::read_to_string(path).unwrap();

        let is_contains_data_root = re.is_match(&prql);

        // for each of the dialects
        insta::allow_duplicates! {
        for con in &mut connections {
            if !con.dialect.should_run_query(&prql) {
                continue;
            }
            let dialect = con.dialect;
            let options = Options::default().with_target(Sql(Some(dialect)));
            let mut prql = prql.clone();
            if is_contains_data_root {
                prql = re.replace_all(&prql, &con.data_file_root).to_string();
            }
            let mut rows = prql_compiler::compile(&prql, &options)
                .and_then(|sql| Ok(con.protocol.run_query(sql.as_str(), runtime)?))
                .context(format!("Executing {test_name} for {dialect}"))
                .unwrap();

            // TODO: I think these could possibly be moved to the DbConnection impls
            crate::dbs::replace_booleans(&mut rows);
            crate::dbs::remove_trailing_zeros(&mut rows);

            let result = rows
                .iter()
                // Make a CSV so it's easier to compare
                .map(|r| r.iter().join(","))
                .join("\n");

            // Add message so we know which dialect fails.
            insta::with_settings!({
                description=>format!("# Running on dialect `{}`\n\n# Query:\n---\n{}", &con.dialect, &prql)
            }, {
                assert_snapshot!("results", &result, &prql);
            })
        }
        }
    })
}
