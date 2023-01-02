use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use strum;

#[derive(
    Debug, PartialEq, Eq, Clone, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
pub enum Target {
    #[strum(serialize = "sql.ansi")]
    Ansi,
    #[strum(serialize = "sql.bigquery")]
    BigQuery,
    #[strum(serialize = "sql.clickhouse")]
    ClickHouse,
    #[strum(serialize = "sql.generic")]
    Generic,
    #[strum(serialize = "sql.hive")]
    Hive,
    #[strum(serialize = "sql.mssql")]
    MsSql,
    #[strum(serialize = "sql.mysql")]
    MySql,
    #[strum(serialize = "sql.postgres")]
    PostgreSql,
    #[strum(serialize = "sql.sqlite")]
    SQLite,
    #[strum(serialize = "sql.snowflake")]
    Snowflake,
}

// Is this the best approach for the Enum / Struct — basically that we have one
// Enum that gets its respective Struct, and then the Struct can also get its
// respective Enum?

impl Target {
    pub fn handler(&self) -> Box<dyn TargetHandler> {
        match self {
            Target::MsSql => Box::new(MsSqlTarget),
            Target::MySql => Box::new(MySqlTarget),
            Target::BigQuery => Box::new(BigQueryTarget),
            Target::ClickHouse => Box::new(ClickHouseTarget),
            _ => Box::new(GenericTarget),
        }
    }
}

impl Default for Target {
    fn default() -> Self {
        Target::Generic
    }
}

pub struct GenericTarget;
pub struct MySqlTarget;
pub struct MsSqlTarget;
pub struct BigQueryTarget;
pub struct ClickHouseTarget;

pub trait TargetHandler {
    fn target(&self) -> Target;
    fn use_top(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
    }
}

impl TargetHandler for GenericTarget {
    fn target(&self) -> Target {
        Target::Generic
    }
}

impl TargetHandler for MsSqlTarget {
    fn target(&self) -> Target {
        Target::MsSql
    }
    fn use_top(&self) -> bool {
        true
    }
}

impl TargetHandler for MySqlTarget {
    fn target(&self) -> Target {
        Target::MySql
    }
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl TargetHandler for ClickHouseTarget {
    fn target(&self) -> Target {
        Target::ClickHouse
    }
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl TargetHandler for BigQueryTarget {
    fn target(&self) -> Target {
        Target::BigQuery
    }
    fn ident_quote(&self) -> char {
        '`'
    }
}
