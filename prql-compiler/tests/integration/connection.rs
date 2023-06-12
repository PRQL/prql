use regex::Regex;
use std::env::current_dir;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use mysql::prelude::Queryable;
use mysql::Value;
use once_cell::sync::Lazy;
use pg_bigdecimal::PgNumeric;
use postgres::types::Type;
use prql_compiler::sql::Dialect;
use tiberius::numeric::BigDecimal;
use tiberius::time::time::PrimitiveDateTime;
use tiberius::*;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_util::compat::Compat;

pub type Row = Vec<String>;

pub struct DBConnection {
    pub protocol: Protocol,
    pub dialect: Dialect,
}

pub enum Protocol {
    MySql(mysql::Pool),
    Postgres(postgres::Client),
    Sqlite(rusqlite::Connection),
    SqlServer(tiberius::Client<Compat<TcpStream>>),
    DuckDb(duckdb::Connection),
}

impl DBConnection {
    pub fn new(dialect: Dialect) -> Result<Self> {
        let protocol = match dialect {
            Dialect::DuckDb => {
                let conn = duckdb::Connection::open_in_memory()?;
                Protocol::DuckDb(conn)
            }
            Dialect::SQLite => {
                let conn = rusqlite::Connection::open_in_memory()?;
                Protocol::Sqlite(conn)
            }
            Dialect::MySql => {
                let conn = mysql::Pool::new("mysql://root:root@localhost:3306/dummy")?;
                Protocol::MySql(conn)
            }
            Dialect::Postgres => {
                let conn = postgres::Client::connect(
                    "host=localhost user=root password=root dbname=dummy",
                    postgres::NoTls,
                )?;
                Protocol::Postgres(conn)
            }
            Dialect::MsSql => {
                let mut config = Config::new();
                config.host("127.0.0.1");
                config.port(1433);
                config.trust_cert();
                config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));
                use tokio_util::compat::TokioAsyncWriteCompatExt;
                let conn = RUNTIME.block_on(async {
                    let tcp = TcpStream::connect(config.get_addr()).await?;
                    tcp.set_nodelay(true).unwrap();
                    tiberius::Client::connect(config, tcp.compat_write()).await
                })?;
                Protocol::SqlServer(conn)
            }
            Dialect::ClickHouse => {
                let conn = mysql::Pool::new("mysql://default:@localhost:9004/dummy")?;
                Protocol::MySql(conn)
            }
            _ => unimplemented!("integration is not implemented for {:?}", dialect),
        };
        Ok(DBConnection { dialect, protocol })
    }
    pub fn run_query(&mut self, sql: &str, runtime: &Runtime) -> Result<Vec<Row>> {
        match self.protocol {
            Protocol::DuckDb(_) => {
                let conn = match &mut self.protocol {
                    Protocol::DuckDb(conn) => conn,
                    _ => unreachable!(),
                };
                let mut statement = conn.prepare(sql)?;
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
                            duckdb::types::ValueRef::Null => "".to_string(),
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
                            duckdb::types::ValueRef::Timestamp(u, v) => format!("{} {:?}", v, u),
                            duckdb::types::ValueRef::Text(v) => {
                                String::from_utf8(v.to_vec()).unwrap()
                            }
                            duckdb::types::ValueRef::Blob(_) => "BLOB".to_string(),
                            duckdb::types::ValueRef::Date32(v) => v.to_string(),
                            duckdb::types::ValueRef::Time64(u, v) => format!("{} {:?}", v, u),
                        };
                        columns.push(value);
                    }
                    vec.push(columns)
                }
                Ok(vec)
            }
            Protocol::Sqlite(_) => {
                let conn = match &mut self.protocol {
                    Protocol::Sqlite(conn) => conn,
                    _ => unreachable!(),
                };
                let mut statement = conn.prepare(sql)?;
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
                            rusqlite::types::ValueRef::Null => "".to_string(),
                            rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                            rusqlite::types::ValueRef::Real(v) => v.to_string(),
                            rusqlite::types::ValueRef::Text(v) => {
                                String::from_utf8(v.to_vec()).unwrap()
                            }
                            rusqlite::types::ValueRef::Blob(_) => "BLOB".to_string(),
                        };
                        columns.push(value);
                    }
                    vec.push(columns);
                }
                Ok(vec)
            }
            Protocol::MySql(_) => {
                let mut conn = match &mut self.protocol {
                    Protocol::MySql(pool) => pool.get_conn()?,
                    _ => unreachable!(),
                };
                let rows: Vec<mysql::Row> = conn.query(sql)?;
                let mut vec = vec![];
                for row in rows.into_iter() {
                    let mut columns = vec![];
                    for v in row.unwrap() {
                        let value = match v {
                            Value::NULL => "".to_string(),
                            Value::Bytes(v) => {
                                String::from_utf8(v).unwrap_or_else(|_| "BLOB".to_string())
                            }
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
            Protocol::Postgres(_) => {
                let conn = match &mut self.protocol {
                    Protocol::Postgres(conn) => conn,
                    _ => unreachable!(),
                };
                let rows = conn.query(sql, &[])?;
                let mut vec = vec![];
                for row in rows.into_iter() {
                    let mut columns = vec![];
                    for i in 0..row.len() {
                        let col = &(*row.columns())[i];
                        let value = match col.type_() {
                            &Type::BOOL => (row.get::<usize, bool>(i)).to_string(),
                            &Type::INT4 => match row.try_get::<usize, i32>(i) {
                                Ok(v) => v.to_string(),
                                Err(_) => "".to_string(),
                            },
                            &Type::INT8 => match row.try_get::<usize, i64>(i) {
                                Ok(v) => v.to_string(),
                                Err(_) => "".to_string(),
                            },
                            &Type::TEXT | &Type::VARCHAR | &Type::JSON | &Type::JSONB => {
                                match row.try_get::<usize, String>(i) {
                                    Ok(v) => v,
                                    // handle null
                                    Err(_) => "".to_string(),
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
            Protocol::SqlServer(_) => runtime.block_on(async {
                let conn = match &mut self.protocol {
                    Protocol::SqlServer(conn) => conn,
                    _ => unreachable!(),
                };
                let mut stream = conn.query(sql, &[]).await?;
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
                            ColumnType::Null => "".to_string(),
                            ColumnType::Bit => String::from(row.get::<&str, usize>(i).unwrap()),
                            ColumnType::Intn | ColumnType::Int4 => row
                                .get::<i32, usize>(i)
                                .map(|i| i.to_string())
                                .unwrap_or_else(|| "".to_string()),
                            ColumnType::Floatn => vec![
                                row.try_get::<f32, usize>(i).map(|o| o.map(|n| n as f64)),
                                row.try_get::<f64, usize>(i),
                            ]
                            .into_iter()
                            .find(|r| r.is_ok())
                            .unwrap()
                            .unwrap()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| "".to_string()),
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
            }),
        }
    }
    pub fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        match self.dialect {
            Dialect::DuckDb => {
                let path = get_path_for_table(csv_name);
                let path = path.display().to_string().replace('"', "");
                self.run_query(
                    &format!("COPY {csv_name} FROM '{path}' (AUTO_DETECT TRUE);"),
                    runtime,
                )
                .unwrap();
            }
            Dialect::SQLite => {
                let path = get_path_for_table(csv_name);
                let mut reader = csv::ReaderBuilder::new()
                    .has_headers(true)
                    .from_path(path)
                    .unwrap();
                let headers = reader
                    .headers()
                    .unwrap()
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();
                for result in reader.records() {
                    let r = result.unwrap();
                    let q = format!(
                        "INSERT INTO {csv_name} ({}) VALUES ({})",
                        headers.iter().join(","),
                        r.iter()
                            .map(|s| if s.is_empty() {
                                "null".to_string()
                            } else {
                                format!("\"{}\"", s.replace('"', "\"\""))
                            })
                            .join(",")
                    );
                    self.run_query(q.as_str(), runtime).unwrap();
                }
            }
            Dialect::Postgres => {
                self.run_query(
                    &format!(
                        "COPY {csv_name} FROM '/tmp/chinook/{csv_name}.csv' DELIMITER ',' CSV HEADER;"
                    ),
                    runtime,
                )
                .unwrap();
            }
            Dialect::MySql => {
                // hacky hack for MySQL
                // MySQL needs a special character in csv that means NULL (https://stackoverflow.com/a/2675493)
                // 1. read the csv
                // 2. create a copy with the special character
                // 3. import the data and remove the copy
                let old_path = get_path_for_table(csv_name);
                let mut new_path = old_path.clone();
                new_path.pop();
                new_path.push(format!("{csv_name}.my.csv").as_str());
                let mut file_content = fs::read_to_string(old_path).unwrap();
                file_content = file_content.replace(",,", ",\\N,").replace(",\n", ",\\N\n");
                fs::write(&new_path, file_content).unwrap();
                let query_result = self.run_query(&format!("LOAD DATA INFILE '/tmp/chinook/{csv_name}.my.csv' INTO TABLE {csv_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;"), runtime);
                fs::remove_file(&new_path).unwrap();
                query_result.unwrap();
            }
            Dialect::MsSql => {
                self.run_query(&format!("BULK INSERT {csv_name} FROM '/tmp/chinook/{csv_name}.csv' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');"), runtime).unwrap();
            }
            Dialect::ClickHouse => {
                self.run_query(
                    &format!(
                        "INSERT INTO {csv_name} FROM INFILE '/tmp/chinook/{csv_name}.csv' FORMAT CSV"
                    ),
                    runtime,
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
    }
    pub fn modify_sql(&self, sql: String) -> String {
        match self.dialect {
            Dialect::DuckDb => sql.replace("REAL", "DOUBLE"),
            Dialect::Postgres => sql.replace("REAL", "DOUBLE PRECISION"),
            Dialect::MySql => sql.replace("TIMESTAMP", "DATETIME"),
            Dialect::ClickHouse => {
                let re = Regex::new(r"(?s)\)$").unwrap();
                re.replace(&sql, r") ENGINE = Memory")
                    .replace("TIMESTAMP", "DATETIME")
                    .replace("REAL", "DOUBLE")
            }
            Dialect::MsSql => sql
                .replace("TIMESTAMP", "DATETIME")
                .replace("REAL", "FLOAT(53)")
                .replace(" AS TEXT", " AS VARCHAR"),
            _ => sql,
        }
    }
}

fn get_path_for_table(csv_name: &str) -> PathBuf {
    let mut path = current_dir().unwrap();
    path.extend([
        "tests",
        "integration",
        "data",
        "chinook",
        format!("{csv_name}.csv").as_str(),
    ]);
    path
}

static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create global runtime"));
