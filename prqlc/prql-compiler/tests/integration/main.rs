#![cfg(not(target_family = "wasm"))]
#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

use std::{env, fs};

use anyhow::Context;
use insta::{assert_snapshot, glob};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;

use connection::{DbConnection, DbProtocol, Row};
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

const LOCAL_CHINOOK_DIR: &str = "tests/integration/data/chinook";

fn compile(prql: &str, target: Target) -> Result<String, prql_compiler::ErrorMessages> {
    prql_compiler::compile(prql, &Options::default().no_signature().with_target(target))
}

trait IntegrationTest {
    fn should_run_query(&self, prql: &str) -> bool;
    fn get_connection(&self) -> Option<DbConnection>;
    // We sometimes want to modify the SQL `INSERT` query (we don't modify the
    // SQL `SELECT` query)
    fn import_csv(&mut self, protocol: &mut dyn DbProtocol, csv_path: &str, runtime: &Runtime);
    fn modify_sql(&self, sql: String) -> String;
}

impl DbConnection {
    fn setup_connection(&mut self, runtime: &Runtime) {
        let setup = include_str!("data/chinook/schema.sql");
        setup
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .for_each(|s| {
                self.protocol
                    .run_query(self.dialect.modify_sql(s.to_string()).as_str(), runtime)
                    .unwrap();
            });
        for csv in glob::glob(format!("{}/*.csv", LOCAL_CHINOOK_DIR).as_str()).unwrap() {
            let csv_path = format!(
                "{}/{}",
                self.data_file_root,
                csv.unwrap().file_name().unwrap().to_str().unwrap()
            );
            self.dialect
                .import_csv(&mut *self.protocol, &csv_path, runtime);
        }
    }
}

impl IntegrationTest for Dialect {
    // If it's supported, test unless it has `duckdb:skip`. If it's not
    // supported, test only if it has `duckdb:test`.
    fn should_run_query(&self, prql: &str) -> bool {
        match self.support_level() {
            SupportLevel::Supported => {
                !prql.contains(format!("{}:skip", self.to_string().to_lowercase()).as_str())
            }
            SupportLevel::Unsupported => {
                prql.contains(format!("{}:test", self.to_string().to_lowercase()).as_str())
            }
            SupportLevel::Nascent => false,
        }
    }

    fn get_connection(&self) -> Option<DbConnection> {
        #[cfg(feature = "test-dbs-external")]
        let external_db_default_chinook_dir = "/tmp/chinook".to_string();
        match self {
            Dialect::DuckDb => Some(DbConnection {
                dialect: Dialect::DuckDb,
                protocol: Box::new(duckdb::Connection::open_in_memory().unwrap()),
                data_file_root: LOCAL_CHINOOK_DIR.to_string(),
            }),
            Dialect::SQLite => Some(DbConnection {
                dialect: Dialect::SQLite,
                protocol: Box::new(rusqlite::Connection::open_in_memory().unwrap()),
                data_file_root: LOCAL_CHINOOK_DIR.to_string(),
            }),

            #[cfg(feature = "test-dbs-external")]
            Dialect::Postgres => Some(DbConnection {
                dialect: Dialect::Postgres,
                protocol: Box::new(
                    postgres::Client::connect(
                        "host=localhost user=root password=root dbname=dummy",
                        postgres::NoTls,
                    )
                    .unwrap(),
                ),
                data_file_root: external_db_default_chinook_dir,
            }),
            #[cfg(feature = "test-dbs-external")]
            Dialect::GlareDb => Some(DbConnection {
                dialect: Dialect::GlareDb,
                protocol: Box::new(
                    postgres::Client::connect(
                        "host=localhost user=glaredb dbname=glaredb port=6543",
                        postgres::NoTls,
                    )
                    .unwrap(),
                ),
                data_file_root: external_db_default_chinook_dir,
            }),
            #[cfg(feature = "test-dbs-external")]
            Dialect::MySql => Some(DbConnection {
                dialect: Dialect::MySql,
                protocol: Box::new(
                    mysql::Pool::new("mysql://root:root@localhost:3306/dummy").unwrap(),
                ),
                data_file_root: external_db_default_chinook_dir,
            }),
            #[cfg(feature = "test-dbs-external")]
            Dialect::ClickHouse => Some(DbConnection {
                dialect: Dialect::ClickHouse,
                protocol: Box::new(
                    mysql::Pool::new("mysql://default:@localhost:9004/dummy").unwrap(),
                ),
                data_file_root: "chinook".to_string(),
            }),
            #[cfg(feature = "test-dbs-external")]
            Dialect::MsSql => {
                use tiberius::{AuthMethod, Client, Config};
                use tokio::net::TcpStream;
                use tokio_util::compat::TokioAsyncWriteCompatExt;

                let mut config = Config::new();
                config.host("localhost");
                config.port(1433);
                config.trust_cert();
                config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

                Some(DbConnection {
                    dialect: Dialect::MsSql,
                    protocol: Box::new(
                        RUNTIME
                            .block_on(async {
                                let tcp = TcpStream::connect(config.get_addr()).await?;
                                tcp.set_nodelay(true).unwrap();
                                Client::connect(config, tcp.compat_write()).await
                            })
                            .unwrap(),
                    ),
                    data_file_root: external_db_default_chinook_dir,
                })
            }
            _ => None,
        }
    }
    fn import_csv(&mut self, protocol: &mut dyn DbProtocol, csv_path: &str, runtime: &Runtime) {
        let csv_path_binding = std::path::PathBuf::from(csv_path);
        let csv_name = csv_path_binding.file_stem().unwrap().to_str().unwrap();
        match self {
            Dialect::DuckDb => {
                protocol
                    .run_query(
                        &format!("COPY {csv_name} FROM '{csv_path}' (AUTO_DETECT TRUE);"),
                        runtime,
                    )
                    .unwrap();
            }
            Dialect::SQLite => {
                let mut reader = csv::ReaderBuilder::new()
                    .has_headers(true)
                    .from_path(csv_path)
                    .unwrap();
                let headers = reader
                    .headers()
                    .unwrap()
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();
                for result in reader.records() {
                    let r = result.unwrap();
                    let q = format!(
                        "INSERT INTO {csv_name} ({}) VALUES ({})",
                        headers.iter().join(","),
                        r.iter()
                            .map(|s| if s.is_empty() {
                                "null".to_string()
                            } else {
                                format!("\"{}\"", s.replace('"', "\"\""))
                            })
                            .join(",")
                    );
                    protocol.run_query(q.as_str(), runtime).unwrap();
                }
            }
            Dialect::Postgres => {
                protocol
                    .run_query(
                        &format!("COPY {csv_name} FROM '{csv_path}' DELIMITER ',' CSV HEADER;"),
                        runtime,
                    )
                    .unwrap();
            }
            Dialect::GlareDb => {
                protocol
                    .run_query(
                        &format!("INSERT INTO {csv_name} SELECT * FROM '{csv_path}'"),
                        runtime,
                    )
                    .unwrap();
            }
            Dialect::MySql => {
                // hacky hack for MySQL
                // MySQL needs a special character in csv that means NULL (https://stackoverflow.com/a/2675493)
                // 1. read the csv
                // 2. create a copy with the special character
                // 3. import the data and remove the copy
                let local_csv_path = format!(
                    "{}/{}",
                    LOCAL_CHINOOK_DIR,
                    csv_path_binding.file_name().unwrap().to_str().unwrap()
                );
                let local_old_path = std::path::PathBuf::from(local_csv_path);
                let mut local_new_path = local_old_path.clone();
                local_new_path.pop();
                local_new_path.push(format!("{csv_name}.my.csv").as_str());
                let mut file_content = fs::read_to_string(local_old_path).unwrap();
                file_content = file_content.replace(",,", ",\\N,").replace(",\n", ",\\N\n");
                fs::write(&local_new_path, file_content).unwrap();
                let query_result = protocol.run_query(
                    &format!(
                        "LOAD DATA INFILE '{}' INTO TABLE {csv_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;",
                        &csv_path_binding.parent().unwrap().join(local_new_path.file_name().unwrap()).to_str().unwrap()
            ), runtime);
                fs::remove_file(&local_new_path).unwrap();
                query_result.unwrap();
            }
            Dialect::ClickHouse => {
                protocol
                    .run_query(
                        &format!("INSERT INTO {csv_name} SELECT * FROM file('{csv_path}')"),
                        runtime,
                    )
                    .unwrap();
            }
            Dialect::MsSql => {
                protocol.run_query(&format!("BULK INSERT {csv_name} FROM '{csv_path}' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');"), runtime).unwrap();
            }
            _ => unreachable!(),
        }
    }
    fn modify_sql(&self, sql: String) -> String {
        match self {
            Dialect::DuckDb => sql.replace("REAL", "DOUBLE"),
            Dialect::Postgres => sql.replace("REAL", "DOUBLE PRECISION"),
            Dialect::GlareDb => sql.replace("REAL", "DOUBLE PRECISION"),
            Dialect::MySql => sql.replace("TIMESTAMP", "DATETIME"),
            Dialect::ClickHouse => {
                let re = Regex::new(r"(?s)\)$").unwrap();
                re.replace(&sql, r") ENGINE = Memory")
                    .replace("TIMESTAMP", "DATETIME64")
                    .replace("REAL", "DOUBLE")
                    .replace("VARCHAR(255)", "Nullable(String)")
            }
            Dialect::MsSql => sql
                .replace("TIMESTAMP", "DATETIME")
                .replace("REAL", "FLOAT(53)")
                .replace(" AS TEXT", " AS VARCHAR"),
            _ => sql,
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
            replace_booleans(&mut rows);
            remove_trailing_zeros(&mut rows);

            let result = rows
                .iter()
                // Make a CSV so it's easier to compare
                .map(|r| r.iter().join(","))
                .join("\n");

            // Add message so we know which dialect fails.
            insta::with_settings!({
                description=>format!("# Running on dialect `{}`\n\n# Query:\n---\n{}", &con.dialect, &prql)
            }, {
                assert_snapshot!("results", &result);
            })
        }
        }
    })
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
