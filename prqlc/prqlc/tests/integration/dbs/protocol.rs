use anyhow::Result;

use super::Row;

pub(crate) trait DbProtocol: Send {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>>;
}

impl DbProtocol for rusqlite::Connection {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        use rusqlite::types::ValueRef;
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

impl DbProtocol for duckdb::Connection {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        use duckdb::types::ValueRef;
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

#[cfg(feature = "test-dbs-external")]
mod external {
    use anyhow::bail;
    use std::sync::OnceLock;
    use std::time::SystemTime;

    use super::*;

    impl DbProtocol for mysql::Pool {
        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            use mysql::prelude::Queryable;

            let mut conn = self.get_conn()?;
            let rows: Vec<mysql::Row> = conn.query(sql)?;
            let mut vec = vec![];
            for row in rows {
                let mut columns = vec![];
                for v in row.unwrap() {
                    let value = match v {
                        mysql::Value::NULL => String::new(),
                        mysql::Value::Bytes(v) => {
                            String::from_utf8(v).unwrap_or_else(|_| "BLOB".to_string())
                        }
                        mysql::Value::Int(v) => v.to_string(),
                        mysql::Value::UInt(v) => v.to_string(),
                        mysql::Value::Float(v) => v.to_string(),
                        mysql::Value::Double(v) => v.to_string(),
                        typ => bail!("mysql type {:?}", typ),
                    };
                    columns.push(value);
                }
                vec.push(columns);
            }
            Ok(vec)
        }
    }

    impl DbProtocol for tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>> {
        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            fn runtime() -> &'static tokio::runtime::Runtime {
                static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
                RUNTIME.get_or_init(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                })
            }

            runtime().block_on(async {
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
                            tiberius::ColumnType::Null => String::new(),
                            tiberius::ColumnType::Bit => {
                                String::from(row.get::<&str, usize>(i).unwrap())
                            }
                            tiberius::ColumnType::Intn | tiberius::ColumnType::Int4 => {
                                row.get::<i32, usize>(i).map(|i| i.to_string()).unwrap()
                            }
                            tiberius::ColumnType::Floatn
                            | tiberius::ColumnType::Float4
                            | tiberius::ColumnType::Float8 => row
                                .try_get::<f64, usize>(i)
                                .or_else(|_| row.try_get::<f32, usize>(i).map(|v| v.map(f64::from)))
                                .unwrap()
                                .unwrap()
                                .to_string(),
                            tiberius::ColumnType::Numericn | tiberius::ColumnType::Decimaln => row
                                .get::<tiberius::numeric::BigDecimal, usize>(i)
                                .map(|d| d.normalized())
                                .unwrap()
                                .to_string(),
                            tiberius::ColumnType::BigVarChar | tiberius::ColumnType::NVarchar => {
                                String::from(row.get::<&str, usize>(i).unwrap_or(""))
                            }
                            tiberius::ColumnType::Datetimen => row
                                .get::<tiberius::time::time::PrimitiveDateTime, usize>(i)
                                .unwrap()
                                .to_string(),
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

    impl DbProtocol for postgres::Client {
        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            use chrono::{DateTime, Utc};

            let rows = self.query(sql, &[])?;
            let mut vec = vec![];
            for row in rows {
                let mut columns = vec![];
                for i in 0..row.len() {
                    let col = &(*row.columns())[i];
                    let value = match col.type_() {
                        &postgres::types::Type::BOOL => (row.get::<usize, bool>(i)).to_string(),
                        &postgres::types::Type::INT4 => match row.try_get::<usize, i32>(i) {
                            Ok(v) => v.to_string(),
                            Err(_) => String::new(),
                        },
                        &postgres::types::Type::INT8 => match row.try_get::<usize, i64>(i) {
                            Ok(v) => v.to_string(),
                            Err(_) => String::new(),
                        },
                        &postgres::types::Type::TEXT
                        | &postgres::types::Type::VARCHAR
                        | &postgres::types::Type::JSON
                        | &postgres::types::Type::JSONB => {
                            match row.try_get::<usize, String>(i) {
                                Ok(v) => v,
                                // handle null
                                Err(_) => String::new(),
                            }
                        }
                        &postgres::types::Type::FLOAT4 => (row.get::<usize, f32>(i)).to_string(),
                        &postgres::types::Type::FLOAT8 => (row.get::<usize, f64>(i)).to_string(),
                        &postgres::types::Type::NUMERIC => row
                            .get::<usize, pg_bigdecimal::PgNumeric>(i)
                            .n
                            .map(|d| d.normalized())
                            .unwrap()
                            .to_string(),
                        &postgres::types::Type::TIMESTAMPTZ | &postgres::types::Type::TIMESTAMP => {
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
}
