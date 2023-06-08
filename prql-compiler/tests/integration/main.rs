#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeMap;
use std::fmt::Write;
use std::{env, fs};

use anyhow::Context;
use insta::{assert_snapshot, glob};
use once_cell::sync::Lazy;
use regex::Regex;
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;

use connection::*;
use prql_compiler::{sql::Dialect, sql::SupportLevel, Target::Sql};
use prql_compiler::{Options, Target};

mod connection;

// This is copy-pasted from `test.rs` in prql-compiler. Ideally we would have a
// canonical set of examples between both, which this integration test would use
// only for integration tests, and the other test would use for checking the
// SQL. But at the moment we're only using these examples here, and we want to
// test the SQL, so we copy-paste the function here.

// TODO: an relatively easy thing to do would be to use these as the canonical
// examples in the book, and then we get this for free.

fn compile(prql: &str, target: Target) -> Result<String, prql_compiler::ErrorMessages> {
    prql_compiler::compile(prql, &Options::default().no_signature().with_target(target))
}

trait IntegrationTest {
    fn should_run_query(&self, prql: &str) -> bool;
    fn get_connection(&self) -> Option<Box<dyn DBConnection>>;
}

impl IntegrationTest for Dialect {
    fn should_run_query(&self, prql: &str) -> bool {
        !prql.contains(format!("skip_{}", self.to_string().to_lowercase()).as_str())
    }

    fn get_connection(&self) -> Option<Box<dyn DBConnection>> {
        match self {
            Dialect::DuckDb => Some(Box::new(duckdb::Connection::open_in_memory().unwrap())),
            Dialect::SQLite => Some(Box::new(rusqlite::Connection::open_in_memory().unwrap())),

            #[cfg(feature = "test-external-dbs")]
            Dialect::Postgres => {
                use postgres::NoTls;
                Some(Box::new(
                    postgres::Client::connect(
                        "host=localhost user=root password=root dbname=dummy",
                        NoTls,
                    )
                    .unwrap(),
                ))
            }

            #[cfg(feature = "test-external-dbs")]
            Dialect::MySql => Some(Box::new(
                mysql::Pool::new("mysql://root:root@localhost:3306/dummy").unwrap(),
            )),
            #[cfg(feature = "test-external-dbs")]
            Dialect::MsSql => Some({
                use tiberius::{AuthMethod, Client, Config};
                use tokio::net::TcpStream;
                use tokio_util::compat::TokioAsyncWriteCompatExt;

                let mut config = Config::new();
                config.host("127.0.0.1");
                config.port(1433);
                config.trust_cert();
                config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

                Box::new(
                    RUNTIME
                        .block_on(async {
                            let tcp = TcpStream::connect(config.get_addr()).await?;
                            tcp.set_nodelay(true).unwrap();
                            Client::connect(config, tcp.compat_write()).await
                        })
                        .unwrap(),
                )
            }),
            _ => None,
        }
    }
}

#[test]
fn test_sql_examples_generic() {
    // We're currently not testing for each dialect, as it's a lot of snapshots.
    // We can consider doing that if helpful.
    glob!("queries/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();
        assert_snapshot!(
            "sql",
            compile(&prql, Target::Sql(Some(Dialect::Generic))).unwrap(),
            &prql
        )
    });
}

#[test]
fn test_fmt_examples() {
    glob!("queries/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();

        let pl = prql_compiler::prql_to_pl(&prql).unwrap();
        let formatted = prql_compiler::pl_to_prql(pl).unwrap();

        assert_snapshot!("fmt", &formatted, &prql)
    });
}

#[test]
fn test_rdbms() {
    let runtime = &*RUNTIME;

    let mut connections: Vec<Box<dyn DBConnection>> = Dialect::iter()
        .filter(|dialect| matches!(dialect.support_level(), SupportLevel::Supported))
        .filter_map(|dialect| dialect.get_connection())
        .collect();

    connections.iter_mut().for_each(|con| {
        setup_connection(&mut **con, runtime);
    });

    // for each of the queries
    glob!("queries/**/*.prql", |path| {
        let test_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        // read
        let prql = fs::read_to_string(path).unwrap();

        if prql.contains("skip_test") {
            return;
        }

        let mut results = BTreeMap::new();
        for con in &mut connections {
            let vendor = con.get_dialect().to_string().to_lowercase();
            if !con.get_dialect().should_run_query(&prql) {
                continue;
            }
            let res = run_query(&mut **con, prql.as_str(), runtime);
            let res = res.context(format!("Executing for {vendor}")).unwrap();
            results.insert(vendor, res);
        }

        if results.is_empty() {
            return;
        }

        let first_result = match results.iter().next() {
            Some(v) => v,
            None => return,
        };
        for (k, v) in results.iter().skip(1) {
            pretty_assertions::assert_eq!(
                *first_result.1,
                *v,
                "{} == {}: {test_name}",
                first_result.0,
                k
            );
        }

        let mut result_string = String::new();
        for row in first_result.1 {
            writeln!(&mut result_string, "{}", row.join(",")).unwrap_or_default();
        }
        assert_snapshot!("results", result_string, &prql);
    });
}

fn setup_connection(con: &mut dyn DBConnection, runtime: &Runtime) {
    let setup = include_str!("data/chinook/schema.sql");
    setup
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .for_each(|s| {
            con.run_query(con.modify_sql(s.to_string()).as_str(), runtime)
                .unwrap();
        });
    let tables = [
        "invoices",
        "customers",
        "employees",
        "tracks",
        "albums",
        "genres",
        "playlist_track",
        "playlists",
        "media_types",
        "artists",
        "invoice_items",
    ];
    for table in tables {
        con.import_csv(table, runtime);
    }
}

fn run_query(
    con: &mut dyn DBConnection,
    prql: &str,
    runtime: &Runtime,
) -> anyhow::Result<Vec<Row>> {
    let options = Options::default().with_target(Sql(Some(con.get_dialect())));
    let sql = prql_compiler::compile(prql, &options)?;

    let mut actual_rows = con.run_query(sql.as_str(), runtime)?;
    replace_booleans(&mut actual_rows);
    remove_trailing_zeros(&mut actual_rows);
    Ok(actual_rows)
}

// some sql dialects use 1 and 0 instead of true and false
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

// MySQL may adds 0s to the end of results of `/` operator
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

static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create global runtime"));
