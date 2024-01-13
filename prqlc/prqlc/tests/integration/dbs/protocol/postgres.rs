use std::time::SystemTime;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use pg_bigdecimal::PgNumeric;
use postgres::types::Type;

use super::DbProtocolHandler;
use crate::dbs::Row;

pub fn init(url: &str) -> Box<dyn DbProtocolHandler> {
    Box::new(postgres::Client::connect(url, postgres::NoTls).unwrap())
}

impl DbProtocolHandler for postgres::Client {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        let rows = self.query(sql, &[])?;
        let mut vec = vec![];
        for row in rows {
            let mut columns = vec![];
            for i in 0..row.len() {
                let col = &(*row.columns())[i];
                let value = match col.type_() {
                    &Type::BOOL => (row.get::<usize, bool>(i)).to_string(),
                    &Type::INT4 => match row.try_get::<usize, i32>(i) {
                        Ok(v) => v.to_string(),
                        Err(_) => String::new(),
                    },
                    &Type::INT8 => match row.try_get::<usize, i64>(i) {
                        Ok(v) => v.to_string(),
                        Err(_) => String::new(),
                    },
                    &Type::TEXT | &Type::VARCHAR | &Type::JSON | &Type::JSONB => {
                        match row.try_get::<usize, String>(i) {
                            Ok(v) => v,
                            // handle null
                            Err(_) => String::new(),
                        }
                    }
                    &Type::FLOAT4 => (row.get::<usize, f32>(i)).to_string(),
                    &Type::FLOAT8 => (row.get::<usize, f64>(i)).to_string(),
                    &Type::NUMERIC => row
                        .get::<usize, PgNumeric>(i)
                        .n
                        .map(|d| d.normalized())
                        .unwrap()
                        .to_string(),
                    &Type::TIMESTAMPTZ | &Type::TIMESTAMP => {
                        let time = row.get::<usize, SystemTime>(i);
                        let date_time: DateTime<Utc> = time.into();
                        date_time.to_rfc3339()
                    }
                    typ => bail!("postgres type {:?}", typ),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        Ok(vec)
    }
}
