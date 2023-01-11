use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use strum;

#[derive(
    Debug, PartialEq, Eq, Clone, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
pub enum Dialect {
    #[strum(serialize = "ansi")]
    Ansi,
    #[strum(serialize = "bigquery")]
    BigQuery,
    #[strum(serialize = "clickhouse")]
    ClickHouse,
    #[strum(serialize = "generic")]
    Generic,
    #[strum(serialize = "hive")]
    Hive,
    #[strum(serialize = "mssql")]
    MsSql,
    #[strum(serialize = "mysql")]
    MySql,
    #[strum(serialize = "postgres")]
    PostgreSql,
    #[strum(serialize = "sqlite")]
    SQLite,
    #[strum(serialize = "snowflake")]
    Snowflake,
    #[strum(serialize = "duckdb")]
    DuckDb,
}

// Is this the best approach for the Enum / Struct — basically that we have one
// Enum that gets its respective Struct, and then the Struct can also get its
// respective Enum?

impl Dialect {
    pub fn handler(&self) -> Box<dyn DialectHandler> {
        match self {
            Dialect::MsSql => Box::new(MsSqlDialect),
            Dialect::MySql => Box::new(MySqlDialect),
            Dialect::BigQuery => Box::new(BigQueryDialect),
            Dialect::ClickHouse => Box::new(ClickHouseDialect),
            Dialect::Snowflake => Box::new(SnowflakeDialect),
            Dialect::DuckDb => Box::new(DuckDbDialect),
            _ => Box::new(GenericDialect),
        }
    }
}

impl Default for Dialect {
    fn default() -> Self {
        Dialect::Generic
    }
}

pub struct GenericDialect;
pub struct MySqlDialect;
pub struct MsSqlDialect;
pub struct BigQueryDialect;
pub struct ClickHouseDialect;

pub struct SnowflakeDialect;
pub struct DuckDbDialect;

pub enum ColumnExclude {
    Exclude,
    Except,
}
pub trait DialectHandler {
    fn use_top(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
    }

    fn big_query_quoting(&self) -> bool {
        false
    }

    fn column_exclude(&self) -> Option<ColumnExclude> {
        None
    }
}

impl DialectHandler for GenericDialect {}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }
}

impl DialectHandler for MySqlDialect {
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl DialectHandler for ClickHouseDialect {
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl DialectHandler for BigQueryDialect {
    fn ident_quote(&self) -> char {
        '`'
    }
    fn big_query_quoting(&self) -> bool {
        true
    }
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#select_except
        Some(ColumnExclude::Except)
    }
}

impl DialectHandler for SnowflakeDialect {
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://docs.snowflake.com/en/sql-reference/sql/select.html
        Some(ColumnExclude::Exclude)
    }
}

impl DialectHandler for DuckDbDialect {
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://duckdb.org/2022/05/04/friendlier-sql.html#select--exclude
        Some(ColumnExclude::Exclude)
    }
}
