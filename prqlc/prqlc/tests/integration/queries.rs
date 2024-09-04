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
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use std::{fs, ops::DerefMut};

    use anyhow::Result;
    use prqlc::{sql::SupportLevel, Options, Target};
    use regex::Regex;

    use super::*;
    use crate::dbs::runner::*;
    use crate::dbs::Row;

    fn runners() -> &'static Mutex<Vec<Box<dyn DbTestRunner>>> {
        static RUNNERS: OnceLock<Mutex<Vec<Box<dyn DbTestRunner>>>> = OnceLock::new();
        RUNNERS.get_or_init(|| {
            let mut runners = vec![];

            let local_runners: Vec<Box<dyn DbTestRunner>> = vec![
                Box::new(SQLiteTestRunner::new(
                    "tests/integration/data/chinook".to_string(),
                )),
                Box::new(DuckDbTestRunner::new(
                    "tests/integration/data/chinook".to_string(),
                )),
            ];
            runners.extend(local_runners);

            #[cfg(feature = "test-dbs-external")]
            {
                let external_runners: Vec<Box<dyn DbTestRunner>> = vec![
                    Box::new(PostgresTestRunner::new(
                        "host=localhost user=root password=root dbname=dummy",
                        "/tmp/chinook".to_string(),
                    )),
                    Box::new(MySqlTestRunner::new(
                        "mysql://root:root@localhost:3306/dummy",
                        "/tmp/chinook".to_string(),
                    )),
                    Box::new(ClickHouseTestRunner::new(
                        "mysql://default:@localhost:9004/dummy",
                        "chinook".to_string(),
                    )),
                    Box::new(GlareDbTestRunner::new(
                        "host=localhost user=glaredb dbname=glaredb port=6543",
                        "/tmp/chinook".to_string(),
                    )),
                    Box::new(MsSqlTestRunner::new("/tmp/chinook".to_string())),
                ];
                runners.extend(external_runners);
            }

            Mutex::new({
                runners
                    .into_iter()
                    .filter(|cfg| {
                        matches!(
                            cfg.dialect().support_level(),
                            SupportLevel::Supported | SupportLevel::Unsupported
                        )
                    })
                    .map(|mut runner| {
                        runner.setup();
                        runner
                    })
                    .collect()
            })
        })
    }

    fn should_run_query(dialect: Dialect, prql: &str) -> bool {
        let dialect_str = dialect.to_string().to_lowercase();

        match dialect.support_level() {
            SupportLevel::Supported => !prql.contains(&format!("{dialect_str}:skip")),
            SupportLevel::Unsupported => prql.contains(&format!("{dialect_str}:test")),
            SupportLevel::Nascent => false,
        }
    }

    fn run_query(runner: &mut Box<dyn DbTestRunner>, prql: &str) -> Result<Vec<Row>> {
        let dialect = runner.dialect();
        let options = Options::default().with_target(Target::Sql(Some(dialect)));
        let sql = prqlc::compile(prql, &options)?;

        let mut rows = runner.query(&sql)?;

        replace_booleans(&mut rows);
        remove_trailing_zeros(&mut rows);

        Ok(rows)
    }

    fn replace_booleans(rows: &mut Vec<Row>) {
        for row in rows {
            for col in row {
                if col == "true" {
                    *col = "1".to_string();
                } else if col == "false" {
                    *col = "0".to_string();
                }
            }
        }
    }

    fn remove_trailing_zeros(rows: &mut Vec<Row>) {
        let re = Regex::new(r"^(|-)\d+\.\d+0+$").unwrap();
        for row in rows {
            for col in row {
                if re.is_match(col) {
                    *col = Regex::new(r"0+$").unwrap().replace_all(col, "").to_string();
                }
            }
        }
    }

    fn run(prql_path: &Path) {
        let prql = fs::read_to_string(prql_path).unwrap();
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();

        let data_file_root_keyword = "data_file_root";
        let is_contains_data_root = prql.contains(data_file_root_keyword);

        // for each of the runners
        let mut results = Vec::new();
        for runner in runners().lock().unwrap().deref_mut() {
            if !should_run_query(runner.as_ref().dialect(), &prql) {
                continue;
            }
            let mut prql = prql.clone();
            if is_contains_data_root {
                prql = prql.replace(data_file_root_keyword, runner.data_file_root());
            }

            println!("Executing {test_name} for {}", runner.dialect());
            let rows = run_query(runner, &prql).unwrap();
            // convert into ad-hoc CSV
            let result = rows
                .iter()
                .map(|r| r.join(","))
                .collect::<Vec<_>>()
                .join("\n");

            // If we ever have more than one runner per dialect, we can adjust
            // this to use the runner name
            results.push((runner.dialect(), result));
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
    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }
}
