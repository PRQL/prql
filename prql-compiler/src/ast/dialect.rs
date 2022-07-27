use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use strum;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::EnumString, strum::Display)]
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

pub trait DialectHandler {
    fn dialect(&self) -> Dialect;
    fn use_top(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
    }
}

impl DialectHandler for GenericDialect {
    fn dialect(&self) -> Dialect {
        Dialect::Generic
    }
}

impl DialectHandler for MsSqlDialect {
    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }
    fn use_top(&self) -> bool {
        true
    }
}

impl DialectHandler for MySqlDialect {
    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl DialectHandler for BigQueryDialect {
    fn dialect(&self) -> Dialect {
        Dialect::BigQuery
    }
    fn ident_quote(&self) -> char {
        '`'
    }
}
