#![cfg(feature = "test-dbs-external")]

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use mysql::prelude::Queryable;
use mysql::Value;
use pg_bigdecimal::PgNumeric;
use postgres::types::Type;
use std::time::SystemTime;
use tiberius::numeric::BigDecimal;
use tiberius::time::time::PrimitiveDateTime;
use tiberius::ColumnType;
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use super::*;

impl DbProtocol for postgres::Client {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Result<Vec<Row>> {
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

impl DbProtocol for mysql::Pool {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Result<Vec<Row>> {
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

impl DbProtocol for tiberius::Client<Compat<TcpStream>> {
    fn run_query(&mut self, sql: &str, runtime: &Runtime) -> Result<Vec<Row>> {
        runtime.block_on(async {
            let mut stream = self.query(sql, &[]).await?;
            let mut vec = vec![];
            let cols_option = stream.columns().await?;
            if cols_option.is_none() {
                return Ok(vec);
            }
            let cols = cols_option.unwrap().to_vec();
            for row in stream.into_first_result().await.unwrap() {
                let mut columns = vec![];
                for (i, col) in cols.iter().enumerate() {
                    let value = match col.column_type() {
                        ColumnType::Null => String::new(),
                        ColumnType::Bit => String::from(row.get::<&str, usize>(i).unwrap()),
                        ColumnType::Intn | ColumnType::Int4 => row
                            .get::<i32, usize>(i)
                            .map_or_else(String::new, |i| i.to_string()),
                        ColumnType::Floatn => vec![
                            row.try_get::<f32, usize>(i).map(|o| o.map(f64::from)),
                            row.try_get::<f64, usize>(i),
                        ]
                        .into_iter()
                        .find(|r| r.is_ok())
                        .unwrap()
                        .unwrap()
                        .map_or_else(String::new, |i| i.to_string()),
                        ColumnType::Numericn | ColumnType::Decimaln => row
                            .get::<BigDecimal, usize>(i)
                            .map(|d| d.normalized())
                            .unwrap()
                            .to_string(),
                        ColumnType::BigVarChar | ColumnType::NVarchar => {
                            String::from(row.get::<&str, usize>(i).unwrap_or(""))
                        }
                        ColumnType::Datetimen => {
                            row.get::<PrimitiveDateTime, usize>(i).unwrap().to_string()
                        }
                        typ => bail!("mssql type {:?}", typ),
                    };
                    columns.push(value);
                }
                vec.push(columns);
            }

            Ok(vec)
        })
    }
}
