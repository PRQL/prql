use anyhow::{bail, Result};
use mysql::prelude::Queryable;
use mysql::Value;

use super::DbProtocolHandler;
use crate::dbs::Row;

pub fn init(url: &str) -> Box<dyn DbProtocolHandler> {
    Box::new(
        mysql::Pool::new(url).unwrap_or_else(|e| panic!("Failed to connect to {}:\n{}", url, e)),
    )
}

impl DbProtocolHandler for mysql::Pool {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        let mut conn = self.get_conn()?;
        let rows: Vec<mysql::Row> = conn.query(sql)?;
        let mut vec = vec![];
        for row in rows {
            let mut columns = vec![];
            for v in row.unwrap() {
                let value = match v {
                    Value::NULL => String::new(),
                    Value::Bytes(v) => String::from_utf8(v).unwrap_or_else(|_| "BLOB".to_string()),
                    Value::Int(v) => v.to_string(),
                    Value::UInt(v) => v.to_string(),
                    Value::Float(v) => v.to_string(),
                    Value::Double(v) => v.to_string(),
                    typ => bail!("mysql type {:?}", typ),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        Ok(vec)
    }
}
