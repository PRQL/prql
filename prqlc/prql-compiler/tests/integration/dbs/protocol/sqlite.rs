use anyhow::Result;
use rusqlite::types::ValueRef;

use super::DbProtocolHandler;
use crate::dbs::Row;

pub fn init() -> Box<dyn DbProtocolHandler> {
    Box::new(rusqlite::Connection::open_in_memory().unwrap())
}

impl DbProtocolHandler for rusqlite::Connection {
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
                    ValueRef::Integer(v) => v.to_string(),
                    ValueRef::Real(v) => v.to_string(),
                    ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    ValueRef::Blob(_) => "BLOB".to_string(),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        Ok(vec)
    }
}
