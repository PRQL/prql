#![cfg(not(target_family = "wasm"))]
#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

mod protocol;
mod runner;

use anyhow::Result;
use connector_arrow::arrow;
use prqlc::{sql::Dialect, sql::SupportLevel, Options, Target};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub use self::protocol::DbProtocol;
use self::protocol::DbProtocolHandler;
use self::runner::DbTestRunner;

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
            #[cfg(feature = "test-dbs")]
            DbProtocol::DuckDb => protocol::duckdb::init(),

            #[cfg(feature = "test-dbs")]
            DbProtocol::SQLite => protocol::sqlite::init(),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::Postgres { url } => protocol::postgres::init(url),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::MySql { url } => protocol::mysql::init(url),

            #[cfg(feature = "test-dbs-external")]
            DbProtocol::MsSql => protocol::mssql::init(),

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
                self.protocol.execute(&s).unwrap();
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

    pub fn run_query(&mut self, prql: &str) -> Result<arrow::record_batch::RecordBatch> {
        // compile to SQL
        let dialect = self.cfg.dialect;
        let options = Options::default().with_target(Target::Sql(Some(dialect)));
        let sql = prqlc::compile(prql, &options)?;

        // execute
        let res = self.protocol.query(&sql)?;
        Ok(res)
    }
}

/// Converts arrow::RecordBatch into ad-hoc CSV
pub fn batch_to_csv(batch: arrow::record_batch::RecordBatch) -> String {
    let mut res = String::with_capacity((batch.num_rows() + 1) * batch.num_columns() * 20);

    // print header
    /*
    res.push_str(
        batch
            .schema()
            .fields()
            .iter()
            .map(|f| {
                let ty = f.data_type().to_string();
                format!("{} [{ty}]", f.name())
            })
            .join(",")
            .as_str(),
    );
    res.push('\n');
    */

    // convert each column to string
    let mut arrays = Vec::with_capacity(batch.num_columns());
    for col_i in 0..batch.num_columns() {
        let mut array = batch.columns().get(col_i).unwrap().clone();
        if *array.data_type() == arrow::datatypes::DataType::Boolean {
            array = arrow::compute::cast(&array, &arrow::datatypes::DataType::UInt8).unwrap();
        }
        let array = arrow::compute::cast(&array, &arrow::datatypes::DataType::Utf8).unwrap();
        let array = arrow::array::AsArray::as_string::<i32>(&array).clone();
        arrays.push(array);
    }

    let re = Regex::new(r"^-?\d+\.\d*0+$").unwrap();
    for row_i in 0..batch.num_rows() {
        for (i, col) in arrays.iter().enumerate() {
            let mut value = col.value(row_i);

            // HACK: trim trailing 0
            if re.is_match(value) {
                value = value.trim_end_matches('0').trim_end_matches('.');
            }
            res.push_str(value);
            if i < batch.num_columns() - 1 {
                res.push(',');
            } else {
                res.push('\n');
            }
        }
    }

    res.shrink_to_fit();
    res
}
