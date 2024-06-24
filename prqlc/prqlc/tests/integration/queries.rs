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
    use std::sync::Mutex;
    use std::sync::OnceLock;

    use prqlc::sql::SupportLevel;

    use super::*;
    use crate::dbs::{ConnectionCfg, DbConnection, DbProtocol};

    fn connections() -> &'static Mutex<Vec<DbConnection>> {
        static CONNECTIONS: OnceLock<Mutex<Vec<DbConnection>>> = OnceLock::new();
        CONNECTIONS.get_or_init(|| {
            Mutex::new({
                let configs = [
                    ConnectionCfg {
                        dialect: Dialect::SQLite,
                        data_file_root: "tests/integration/data/chinook".to_string(),

                        protocol: DbProtocol::SQLite,
                    },
                    ConnectionCfg {
                        dialect: Dialect::DuckDb,
                        data_file_root: "tests/integration/data/chinook".to_string(),

                        protocol: DbProtocol::DuckDb,
                    },
                    ConnectionCfg {
                        dialect: Dialect::Postgres,
                        data_file_root: "/tmp/chinook".to_string(),

                        protocol: DbProtocol::Postgres {
                            url: "host=localhost user=root password=root dbname=dummy".to_string(),
                        },
                    },
                    ConnectionCfg {
                        dialect: Dialect::MySql,
                        data_file_root: "/tmp/chinook".to_string(),

                        protocol: DbProtocol::MySql {
                            url: "mysql://root:root@localhost:3306/dummy".to_string(),
                        },
                    },
                    ConnectionCfg {
                        dialect: Dialect::ClickHouse,
                        data_file_root: "chinook".to_string(),

                        protocol: DbProtocol::MySql {
                            url: "mysql://default:@localhost:9004/dummy".to_string(),
                        },
                    },
                    ConnectionCfg {
                        dialect: Dialect::GlareDb,
                        data_file_root: "/tmp/chinook".to_string(),

                        protocol: DbProtocol::Postgres {
                            url: "host=localhost user=glaredb dbname=glaredb port=6543".to_string(),
                        },
                    },
                    ConnectionCfg {
                        dialect: Dialect::MsSql,
                        data_file_root: "/tmp/chinook".to_string(),

                        protocol: DbProtocol::MsSql,
                    },
                ];

                let mut connections = Vec::new();
                for cfg in configs {
                    if !matches!(
                        cfg.dialect.support_level(),
                        SupportLevel::Supported | SupportLevel::Unsupported
                    ) {
                        continue;
                    }

                    // The filtering is not a great design, since it doesn't proactively
                    // check that we can get connections; but it's a compromise given we
                    // implement the external_dbs feature using this.
                    let Some(mut connection) = DbConnection::new(cfg) else {
                        continue;
                    };

                    connection.setup();

                    connections.push(connection);
                }
                connections
            })
        })
    }

    test_each_path! { in "./prqlc/prqlc/tests/integration/queries" => run }

    fn run(prql_path: &Path) {
        let test_name = prql_path.file_stem().unwrap().to_str().unwrap();
        let prql = fs::read_to_string(prql_path).unwrap();

        let data_file_root_keyword = "data_file_root";
        let is_contains_data_root = prql.contains(data_file_root_keyword);

        // for each of the connections
        let mut results = Vec::new();
        for con in connections().lock().unwrap().deref_mut() {
            if !con.should_run_query(&prql) {
                continue;
            }

            let mut prql = prql.clone();
            if is_contains_data_root {
                prql = prql.replace(data_file_root_keyword, &con.cfg.data_file_root);
            }

            let dialect = con.cfg.dialect;

            println!("Executing {test_name} for {dialect}");
            let rows = con.run_query(&prql).unwrap();

            // convert into ad-hoc CSV
            let result = rows
                .iter()
                .map(|r| r.join(","))
                .collect::<Vec<_>>()
                .join("\n");

            results.push((dialect, result));
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
