// Re-enable on windows when duckdb supports it
// https://github.com/wangfenjin/duckdb-rs/issues/62
#![cfg(not(any(target_family = "windows", target_family = "wasm")))]

mod connection;

use std::collections::BTreeMap;
use std::fmt::Write;
use std::{env, fs};

use insta::{assert_snapshot, glob};
use postgres::NoTls;
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use prql_compiler::sql::Dialect;
use prql_compiler::Options;
use prql_compiler::Target::Sql;

use crate::connection::*;

// This is copy-pasted from `test.rs` in prql-compiler. Ideally we would have a
// canonical set of examples between both, which this integration test would use
// only for integration tests, and the other test would use for checking the
// SQL. But at the moment we're only using these examples here, and we want to
// test the SQL, so we copy-paste the function here.

// TODO: an relatively easy thing to do would be to use these as the canonical
// examples in the book, and then we get this for free.

fn compile(prql: &str) -> Result<String, prql_compiler::ErrorMessages> {
    prql_compiler::compile(prql, &Options::default().no_signature())
}

#[test]
fn test_sql_examples() {
    glob!("queries/**/*.prql", |path| {
        let sql = fs::read_to_string(path).unwrap();
        assert_snapshot!(
            path.file_name().unwrap().to_string_lossy().to_string(),
            compile(&sql).unwrap(),
            &sql
        )
    });
}

#[test]
fn test_rdbms() {
    let runtime = Runtime::new().unwrap();
    let mut connections = get_connections(&runtime);

    for con in &mut connections {
        setup_connection(con.as_mut(), &runtime);
    }

    // for each of the queries
    glob!("queries/**/*.prql", |path| {
        let test_name = path
            .file_name()
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
            if prql.contains(format!("skip_{}", vendor).as_str()) {
                continue;
            }
            results.insert(vendor, run_query(con.as_mut(), prql.as_str(), &runtime));
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
        assert_snapshot!(result_string);
    });
}

fn get_connections(runtime: &Runtime) -> Vec<Box<dyn DBConnection>> {
    let mut connections: Vec<Box<dyn DBConnection>> = vec![];
    connections.push(Box::new(DuckDBConnection(
        duckdb::Connection::open_in_memory().unwrap(),
    )));
    connections.push(Box::new(SQLiteConnection(
        rusqlite::Connection::open_in_memory().unwrap(),
    )));

    #[cfg(not(feature = "test-external-dbs"))]
    let include_external_dbs = false;
    #[cfg(feature = "test-external-dbs")]
    let include_external_dbs = true;
    if !include_external_dbs {
        return connections;
    }

    connections.push(Box::new(PostgresConnection(
        postgres::Client::connect("host=localhost user=root password=root dbname=dummy", NoTls)
            .unwrap(),
    )));
    connections.push(Box::new(MysqlConnection(
        mysql::Pool::new("mysql://root:root@localhost:3306/dummy").unwrap(),
    )));
    let ms_client = {
        let mut config = Config::new();
        config.host("127.0.0.1");
        config.port(1433);
        config.trust_cert();
        config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

        let client = runtime.block_on(get_client(config.clone()));

        async fn get_client(config: Config) -> tiberius::Result<Client<Compat<TcpStream>>> {
            let tcp = TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true).unwrap();
            Client::connect(config, tcp.compat_write()).await
        }
        client
    }
    .unwrap();
    connections.push(Box::new(MssqlConnection(ms_client)));

    connections
}

fn setup_connection(con: &mut dyn DBConnection, runtime: &Runtime) {
    let setup = include_str!("data/chinook/schema.sql");
    setup
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .for_each(|s| {
            let sql = match con.get_dialect() {
                Dialect::MsSql => s.replace("TIMESTAMP", "DATETIME"),
                Dialect::MySql => s.replace('"', "`").replace("TIMESTAMP", "DATETIME"),
                _ => s.to_string(),
            };
            con.run_query(sql.as_str(), runtime);
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

fn run_query(con: &mut dyn DBConnection, prql: &str, runtime: &Runtime) -> Vec<Row> {
    let options = Options::default().with_target(Sql(Some(con.get_dialect())));
    let sql = prql_compiler::compile(prql, &options).unwrap();
    let mut actual_rows = con.run_query(sql.as_str(), runtime);
    replace_booleans(&mut actual_rows);
    actual_rows
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
