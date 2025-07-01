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
    use std::str::FromStr;

    use strum::VariantNames;

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

        let target = Dialect::VARIANTS
            .iter()
            .find(|d| prql.contains(&format!("default-dialect:{}", d)))
            .and_then(|d| Dialect::from_str(d).ok())
            .unwrap_or(Dialect::Generic);

        let target = Target::Sql(Some(target));
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

    use itertools::Itertools;
    use prqlc::sql::SupportLevel;

    use super::*;
    use crate::dbs::{batch_to_csv, runners};

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

        // for each of the runners, get the query
        let results: Vec<(Dialect, String)> = runners()
            .iter()
            .filter_map(|runner| {
                let mut runner = runner.lock().unwrap();
                let dialect = runner.dialect();
                if !should_run_query(dialect, &prql) {
                    return None;
                }

                eprintln!("Executing {test_name} for {dialect}");

                match runner.query(&prql) {
                    Ok(batch) => {
                        let csv = batch_to_csv(batch);
                        Some(Ok((dialect, csv)))
                    }
                    Err(e) => Some(Err(e)),
                }
            })
            .try_collect()
            .unwrap();

        if results.is_empty() {
            panic!("No valid dialects to run the query at {prql_path:#?} against");
        }

        // similar to `insta::allow_duplicates!`, but with reporting of which two cases are
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
