#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

use anyhow::Result;
use prql_compiler::sql::Dialect;
use tokio::runtime::Runtime;

pub type Row = Vec<String>;

pub struct DbConnection {
    pub protocol: Box<dyn DbProtocol>,
    pub dialect: Dialect,
    pub data_file_root: String,
}

pub trait DbProtocol {
    fn run_query(&mut self, sql: &str, runtime: &Runtime) -> Result<Vec<Row>>;
}

impl DbProtocol for duckdb::Connection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Result<Vec<Row>> {
        let mut statement = self.prepare(sql)?;
        let mut rows = statement.query([])?;
        let mut vec = vec![];
        while let Ok(Some(row)) = rows.next() {
            let mut columns = vec![];
            // row.len() always gives 1. hence this workaround
            for i in 0.. {
                let v_ref = match row.get_ref(i) {
                    Ok(v) => v,
                    Err(_) => {
                        break;
                    }
                };
                let value = match v_ref {
                    duckdb::types::ValueRef::Null => String::new(),
                    duckdb::types::ValueRef::Boolean(v) => v.to_string(),
                    duckdb::types::ValueRef::TinyInt(v) => v.to_string(),
                    duckdb::types::ValueRef::SmallInt(v) => v.to_string(),
                    duckdb::types::ValueRef::Int(v) => v.to_string(),
                    duckdb::types::ValueRef::BigInt(v) => v.to_string(),
                    duckdb::types::ValueRef::HugeInt(v) => v.to_string(),
                    duckdb::types::ValueRef::UTinyInt(v) => v.to_string(),
                    duckdb::types::ValueRef::USmallInt(v) => v.to_string(),
                    duckdb::types::ValueRef::UInt(v) => v.to_string(),
                    duckdb::types::ValueRef::UBigInt(v) => v.to_string(),
                    duckdb::types::ValueRef::Float(v) => v.to_string(),
                    duckdb::types::ValueRef::Double(v) => v.to_string(),
                    duckdb::types::ValueRef::Decimal(v) => v.to_string(),
                    duckdb::types::ValueRef::Timestamp(u, v) => format!("{v} {u:?}"),
                    duckdb::types::ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    duckdb::types::ValueRef::Blob(_) => "BLOB".to_string(),
                    duckdb::types::ValueRef::Date32(v) => v.to_string(),
                    duckdb::types::ValueRef::Time64(u, v) => format!("{v} {u:?}"),
                };
                columns.push(value);
            }
            vec.push(columns)
        }
        Ok(vec)
    }
}

impl DbProtocol for rusqlite::Connection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Result<Vec<Row>> {
        let mut statement = self.prepare(sql)?;
        let mut rows = statement.query([])?;
        let mut vec = vec![];
        while let Ok(Some(row)) = rows.next() {
            let mut columns = vec![];
            // row.len() always gives 1. hence this workaround
            for i in 0.. {
                let v_ref = match row.get_ref(i) {
                    Ok(v) => v,
                    Err(_) => {
                        break;
                    }
                };
                let value = match v_ref {
                    rusqlite::types::ValueRef::Null => String::new(),
                    rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                    rusqlite::types::ValueRef::Real(v) => v.to_string(),
                    rusqlite::types::ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    rusqlite::types::ValueRef::Blob(_) => "BLOB".to_string(),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        Ok(vec)
    }
}
