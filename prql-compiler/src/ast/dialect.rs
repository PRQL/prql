use core::fmt::Debug;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use strum::{self, EnumString};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumString)]
pub enum DialectEnum {
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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct MsSqlDialect;
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct MySqlDialect;
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GenericDialect;

// pub trait Dialect: Debug + Serialize + DeserializeOwned {
pub trait Dialect: Debug + PartialEq {
    fn use_top() -> bool
    where
        Self: Sized,
    {
        true
    }
    // fn default() -> Self {
    //     GenericDialect
    // }
    // fn table_to_sql_cte(&self, table: crate::translator::AtomicTable) -> Result<sql_ast::Cte>;
}

impl Dialect for GenericDialect {}

impl Dialect for MsSqlDialect {
    fn use_top() -> bool {
        false
    }
}
