#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

pub mod duckdb;

#[cfg(feature = "test-dbs-external")]
pub mod mssql;

#[cfg(feature = "test-dbs-external")]
pub mod mysql;

#[cfg(feature = "test-dbs-external")]
pub mod postgres;

pub mod sqlite;

use crate::Result;
use serde::{Deserialize, Serialize};

use super::Row;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "params")]
pub enum DbProtocol {
    DuckDb,
    MsSql,
    MySql { url: String },
    Postgres { url: String },
    SQLite,
}

pub trait DbProtocolHandler: Send {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>>;
}
