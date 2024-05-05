use anyhow::Result;
use duckdb::types::ValueRef;

use super::DbProtocolHandler;
use crate::dbs::Row;

pub fn init() -> Box<dyn DbProtocolHandler> {
    Box::new(duckdb::Connection::open_in_memory().unwrap())
}

impl DbProtocolHandler for duckdb::Connection {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
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
                    ValueRef::Null => String::new(),
                    ValueRef::Boolean(v) => v.to_string(),
                    ValueRef::TinyInt(v) => v.to_string(),
                    ValueRef::SmallInt(v) => v.to_string(),
                    ValueRef::Int(v) => v.to_string(),
                    ValueRef::BigInt(v) => v.to_string(),
                    ValueRef::HugeInt(v) => v.to_string(),
                    ValueRef::UTinyInt(v) => v.to_string(),
                    ValueRef::USmallInt(v) => v.to_string(),
                    ValueRef::UInt(v) => v.to_string(),
                    ValueRef::UBigInt(v) => v.to_string(),
                    ValueRef::Float(v) => v.to_string(),
                    ValueRef::Double(v) => v.to_string(),
                    // We `round` because once in tests a 3 was returned as 3.0,
                    // which breaks the assertions.
                    ValueRef::Decimal(v) => v.round().to_string(),
                    ValueRef::Timestamp(u, v) => format!("{v} {u:?}"),
                    ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    ValueRef::Blob(_) => "BLOB".to_string(),
                    ValueRef::Date32(v) => v.to_string(),
                    ValueRef::Time64(u, v) => format!("{v} {u:?}"),
                    #[allow(unreachable_patterns)]
                    _ => unimplemented!(),
                };
                columns.push(value);
            }
            vec.push(columns)
        }
        Ok(vec)
    }
}
