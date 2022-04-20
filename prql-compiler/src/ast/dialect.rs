use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use strum;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::EnumString, strum::Display)]
pub enum Dialect {
    #[strum(serialize = "ansi")]
    Ansi,
    #[strum(serialize = "click_house")]
    ClickHouse,
    #[strum(serialize = "generic")]
    Generic,
    #[strum(serialize = "hive")]
    Hive,
    #[strum(serialize = "ms", serialize = "microsoft", serialize = "ms_sql_server")]
    MsSql,
    #[strum(serialize = "mysql")]
    MySql,
    #[strum(serialize = "postgresql", serialize = "pg")]
    PostgreSql,
    #[strum(serialize = "sqlite")]
    SQLite,
    #[strum(serialize = "snowflake")]
    Snowflake,
}

impl Dialect {
    pub fn handler(&self) -> Box<dyn DialectHandler> {
        match self {
            Dialect::MsSql => Box::new(MsSqlDialect),
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
pub struct MsSqlDialect;

pub trait DialectHandler {
    fn use_top(&self) -> bool;
}

impl DialectHandler for GenericDialect {
    fn use_top(&self) -> bool {
        false
    }
}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }
}
