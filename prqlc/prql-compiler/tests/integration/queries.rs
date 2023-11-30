#![cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::{env, fs};

use insta::{assert_snapshot, with_settings};

use prql_compiler::sql::Dialect;
use prql_compiler::{Options, Target};
use test_each_file::test_each_path;

mod compile {
    use super::*;

    // If this is giving compilation errors saying `expected identifier, found keyword`,
    // rename the filenames in queries to something that's not a keyword in Rust.
    test_each_path! { in "./prqlc/prql-compiler/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let target = Target::Sql(Some(Dialect::Generic));
        let options = Options::default().no_signature().with_target(target);

        let sql = prql_compiler::compile(&prql, &options).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, &sql, &prql)
        });
    }
}

mod fmt {
    use super::*;

    test_each_path! { in "./prqlc/prql-compiler/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let pl = prql_compiler::prql_to_pl(&prql).unwrap();
        let formatted = prql_compiler::pl_to_prql(pl).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, &formatted, &prql)
        });
    }
}

#[cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]
mod results {
    use super::*;

    use std::{ops::DerefMut, sync::Mutex};

    use anyhow::Context;
    use once_cell::sync::Lazy;
    use prql_compiler::sql::SupportLevel;

    use crate::dbs::{ConnectionCfg, DbConnection};

    static CONNECTIONS: Lazy<Mutex<Vec<DbConnection>>> = Lazy::new(init_connections);

    fn init_connections() -> Mutex<Vec<DbConnection>> {
        let con_dir = "./tests/integration/dbs/connections";

        let mut connections = Vec::new();
        for con_file in fs::read_dir(con_dir).unwrap() {
            let con_file = con_file.unwrap();
            let con_toml = fs::read_to_string(con_file.path()).unwrap();
            let con_cfg: ConnectionCfg = toml::from_str(&con_toml)
                .context(format!("{con_file:?}"))
                .unwrap();

            if !matches!(
                con_cfg.dialect.support_level(),
                SupportLevel::Supported | SupportLevel::Unsupported
            ) {
                continue;
            }

            // The filtering is not a great design, since it doesn't proactively
            // check that we can get connections; but it's a compromise given we
            // implement the external_dbs feature using this.
            let Some(mut connection) = DbConnection::new(con_cfg) else {
                continue;
            };

            connection.setup();

            connections.push(connection);
        }
        Mutex::new(connections)
    }

    test_each_path! { in "./prqlc/prql-compiler/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let data_file_root_keyword = "data_file_root";
        let is_contains_data_root = prql.contains(data_file_root_keyword);

        // for each of the connections
        let mut results = Vec::new();
        for con in CONNECTIONS.lock().unwrap().deref_mut() {
            if !con.should_run_query(&prql) {
                continue;
            }

            let mut prql = prql.clone();
            if is_contains_data_root {
                prql = prql.replace(data_file_root_keyword, &con.cfg.data_file_root);
            }

            let dialect = con.cfg.dialect;

            let rows = con
                .run_query(&prql)
                .context(format!("Executing {test_name} for {dialect}"))
                .unwrap();

            // convert into ad-hoc CSV
            let result = rows
                .iter()
                .map(|r| r.join(","))
                .collect::<Vec<_>>()
                .join("\n");

            results.push((dialect, result));
        }

        // insta::allow_duplicates!, but with reporting of which two cases are not matching.
        let (left_dialect, left_text) = results.swap_remove(0);
        for (right_dialect, right_text) in results {
            similar_asserts::assert_eq!(
                left_text,
                right_text,
                "{} {} {}",
                test_name,
                left_dialect,
                right_dialect
            );
        }

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, left_text, &prql)
        })
    }
}
