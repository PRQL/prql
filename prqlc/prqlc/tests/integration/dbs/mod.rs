#![cfg(not(target_family = "wasm"))]
#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

mod protocol;
mod runner;

use anyhow::Result;
use prqlc::{sql::Dialect, sql::SupportLevel, Options, Target};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub use self::protocol::DbProtocol;
use self::protocol::DbProtocolHandler;
use self::runner::DbTestRunner;
pub type Row = Vec<String>;

pub struct DbConnection {
    /// Configuration parameters
    pub cfg: ConnectionCfg,

    /// Protocol handler (the inner connection)
    pub protocol: Box<dyn DbProtocolHandler>,

    /// Runner that handles DBMS-specific behavior
    runner: Box<dyn DbTestRunner>,
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionCfg {
    /// Wire protocol to use for connecting to the database
    pub protocol: DbProtocol,

    /// SQL dialect to be used
    pub dialect: Dialect,

    /// Path of data file directory within the database container
    pub data_file_root: String,
}

impl DbConnection {
    pub fn new(cfg: ConnectionCfg) -> Option<DbConnection> {
        let protocol = match &cfg.protocol {
            DbProtocol::DuckDb => protocol::duckdb::init(),

            DbProtocol::SQLite => protocol::sqlite::init(),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::Postgres { url } => protocol::postgres::init(url),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::MySql { url } => protocol::mysql::init(url),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::MsSql => protocol::mssql::init(),

            #[allow(unreachable_patterns)]
            _ => return None,
        };

        let runner: Box<dyn DbTestRunner> = match &cfg.dialect {
            Dialect::DuckDb => Box::new(runner::DuckDbTestRunner),
            Dialect::SQLite => Box::new(runner::SQLiteTestRunner),
            Dialect::Postgres => Box::new(runner::PostgresTestRunner),
            Dialect::GlareDb => Box::new(runner::GlareDbTestRunner),
            Dialect::MySql => Box::new(runner::MySqlTestRunner),
            Dialect::ClickHouse => Box::new(runner::ClickHouseTestRunner),
            Dialect::MsSql => Box::new(runner::MsSqlTestRunner),
            _ => return None,
        };

        Some(DbConnection {
            cfg,
            runner,
            protocol,
        })
    }

    pub fn setup(mut self) -> Self {
        let schema = include_str!("../data/chinook/schema.sql");
        schema
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| self.runner.modify_ddl(s.to_string()))
            .for_each(|s| {
                self.protocol.query(&s).unwrap();
            });

        for file_path in glob::glob("tests/integration/data/chinook/*.csv").unwrap() {
            let file_path = file_path.unwrap();
            let stem = file_path.file_stem().unwrap().to_str().unwrap();
            let path = format!("{}/{}.csv", self.cfg.data_file_root, stem);
            self.runner.import_csv(&mut *self.protocol, &path, stem);
        }
        self
    }

    // If it's supported, test unless it has `duckdb:skip`. If it's not
    // supported, test only if it has `duckdb:test`.
    pub fn should_run_query(&self, prql: &str) -> bool {
        let dialect = self.cfg.dialect.to_string().to_lowercase();

        match self.cfg.dialect.support_level() {
            SupportLevel::Supported => !prql.contains(format!("{}:skip", dialect).as_str()),
            SupportLevel::Unsupported => prql.contains(format!("{}:test", dialect).as_str()),
            SupportLevel::Nascent => false,
        }
    }

    pub fn run_query(&mut self, prql: &str) -> Result<Vec<Row>> {
        // compile to SQL
        let dialect = self.cfg.dialect;
        let options = Options::default().with_target(Target::Sql(Some(dialect)));
        let sql = prqlc::compile(prql, &options)?;

        // execute
        let mut rows = self.protocol.query(&sql)?;

        // modify result
        replace_booleans(&mut rows);
        remove_trailing_zeros(&mut rows);

        Ok(rows)
    }
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
