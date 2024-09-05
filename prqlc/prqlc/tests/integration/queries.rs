#![cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::{env, fs};

use insta::assert_debug_snapshot;
use insta::{assert_snapshot, with_settings};
use prqlc::sql::Dialect;
use prqlc::{Options, Target};
use test_each_file::test_each_path;

mod lex {
    use super::*;

    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let tokens = prqlc_parser::lexer::lex_source(&prql).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_debug_snapshot!(test_name, tokens)
        });
    }
}

mod compile {
    use super::*;

    // If this is giving compilation errors saying `expected identifier, found keyword`,
    // rename the filenames in queries to something that's not a keyword in Rust.
    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();
        if prql.contains("generic:skip") {
            return;
        }

        let target = Target::Sql(Some(Dialect::Generic));
        let options = Options::default().no_signature().with_target(target);

        let sql = prqlc::compile(&prql, &options).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, &sql, &prql)
        });
    }
}

mod fmt {
    use super::*;

    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let pl = prqlc::prql_to_pl(&prql).unwrap();
        let formatted = prqlc::pl_to_prql(&pl).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, &formatted, &prql)
        });

        // Check the formatted queries can still compile
        prqlc::prql_to_pl(&formatted).unwrap();
    }
}

#[cfg(feature = "serde_yaml")]
mod debug_lineage {
    use super::*;

    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let pl = prqlc::prql_to_pl(&prql).unwrap();
        let fc = prqlc::internal::pl_to_lineage(pl).unwrap();

        let lineage = serde_yaml::to_string(&fc).unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, &lineage, &prql)
        });
    }
}

#[cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]
mod results {
    use std::ops::DerefMut;

    use anyhow::Result;
    use connector_arrow::arrow::record_batch::RecordBatch;
    use prqlc::sql::SupportLevel;

    use super::*;
    use crate::dbs::{batch_to_csv, run_query, runners};

    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn should_run_query(dialect: Dialect, prql: &str) -> bool {
        let dialect_str = dialect.to_string().to_lowercase();

        match dialect.support_level() {
            SupportLevel::Supported => !prql.contains(&format!("{dialect_str}:skip")),
            SupportLevel::Unsupported => prql.contains(&format!("{dialect_str}:test")),
            SupportLevel::Nascent => false,
        }
    }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let data_file_root_keyword = "data_file_root";
        let is_contains_data_root = prql.contains(data_file_root_keyword);

        // for each of the runners
        let mut results = Vec::new();
        for runner in runners().lock().unwrap().deref_mut() {
            let dialect = runner.dialect();
            if !should_run_query(dialect, &prql) {
                continue;
            }

            let mut prql = prql.clone();
            if is_contains_data_root {
                prql = prql.replace(data_file_root_keyword, runner.data_file_root());
            }

            println!("Executing {test_name} for {dialect}");
            let batch: Result<RecordBatch> = run_query(runner, &prql);

            match batch {
                Ok(batch) => {
                    let csv = batch_to_csv(batch);
                    results.push((dialect, csv));
                }
                Err(e) => {
                    panic!("Error executing query for {dialect}: {e}");
                }
            }
        }

        if results.is_empty() {
            panic!("No valid dialects to run the query at {prql_path:#?} against");
        }

        // insta::allow_duplicates!, but with reporting of which two cases are
        // not matching.
        let ((first_dialect, first_text), others) = results.split_first().unwrap();

        with_settings!({ input_file => prql_path }, {
            assert_snapshot!(test_name, first_text, &prql)
        });

        for (dialect, text) in others {
            similar_asserts::assert_eq!(
                first_text,
                text,
                "{} {} {}",
                test_name,
                first_dialect,
                dialect
            );
        }
    }
}
