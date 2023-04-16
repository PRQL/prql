use std::env::current_dir;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use itertools::Itertools;
use mysql::prelude::Queryable;
use mysql::Value;
use pg_bigdecimal::PgNumeric;
use postgres::types::Type;
use tiberius::numeric::BigDecimal;
use tiberius::time::time::PrimitiveDateTime;
use tiberius::*;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_util::compat::Compat;

use prql_compiler::sql::Dialect;

pub type Row = Vec<String>;

pub struct DuckDBConnection(pub duckdb::Connection);

pub struct SQLiteConnection(pub rusqlite::Connection);

pub struct PostgresConnection(pub postgres::Client);

pub struct MysqlConnection(pub mysql::Pool);

pub struct MssqlConnection(pub tiberius::Client<Compat<TcpStream>>);

pub trait DBConnection {
    // TODO: possibly this should return a Result instead of panicking on
    // errors, and the caller can then state which dialect and query is being
    // run. Currently the errors in CI are a bit confusing (though it's possible
    // to use the line number, so not woeful).
    fn run_query(&mut self, sql: &str, runtime: &Runtime) -> Vec<Row>;

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime);

    fn get_dialect(&self) -> Dialect;
}

impl DBConnection for DuckDBConnection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Vec<Row> {
        let mut statement = self.0.prepare(sql).unwrap();
        let mut rows = statement.query([]).unwrap();
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
                    duckdb::types::ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    duckdb::types::ValueRef::Blob(_) => "BLOB".to_string(),
                    duckdb::types::ValueRef::Date32(v) => v.to_string(),
                    duckdb::types::ValueRef::Time64(u, v) => format!("{} {:?}", v, u),
                };
                columns.push(value);
            }
            vec.push(columns)
        }
        vec
    }

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        let mut path = current_dir().unwrap();
        for p in [
            "tests",
            "integration",
            "data",
            "chinook",
            format!("{csv_name}.csv").as_str(),
        ] {
            path.push(p);
        }
        let path = path.display().to_string().replace('"', "");
        self.run_query(
            &format!("COPY {csv_name} FROM '{path}' (AUTO_DETECT TRUE);"),
            runtime,
        );
    }

    fn get_dialect(&self) -> Dialect {
        Dialect::DuckDb
    }
}

impl DBConnection for SQLiteConnection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Vec<Row> {
        let mut statement = self.0.prepare(sql).unwrap();
        let mut rows = statement.query([]).unwrap();
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
                    rusqlite::types::ValueRef::Text(v) => String::from_utf8(v.to_vec()).unwrap(),
                    rusqlite::types::ValueRef::Blob(_) => "BLOB".to_string(),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        vec
    }

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        let mut path = current_dir().unwrap();
        for p in [
            "tests",
            "integration",
            "data",
            "chinook",
            format!("{csv_name}.csv").as_str(),
        ] {
            path.push(p);
        }
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
            self.run_query(q.as_str(), runtime);
        }
    }

    fn get_dialect(&self) -> Dialect {
        Dialect::SQLite
    }
}

impl DBConnection for PostgresConnection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Vec<Row> {
        let rows = self.0.query(sql, &[]).unwrap();
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
                    &Type::INT8 => (row.get::<usize, i64>(i)).to_string(),
                    &Type::TEXT | &Type::VARCHAR | &Type::JSON | &Type::JSONB => {
                        match row.try_get::<usize, String>(i) {
                            Ok(v) => v,
                            // handle null
                            Err(_) => "".to_string(),
                        }
                    }
                    &Type::FLOAT4 => (row.get::<usize, f32>(i)).to_string(),
                    &Type::FLOAT8 => (row.get::<usize, f64>(i)).to_string(),
                    &Type::NUMERIC => row.get::<usize, PgNumeric>(i).n.unwrap().to_string(),
                    &Type::TIMESTAMPTZ | &Type::TIMESTAMP => {
                        let time = row.get::<usize, SystemTime>(i);
                        let date_time: DateTime<Utc> = time.into();
                        date_time.to_rfc3339()
                    }
                    typ => unimplemented!("postgres type {:?}", typ),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        vec
    }

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        self.run_query(
            &format!(
                "COPY {csv_name} FROM '/tmp/chinook/{csv_name}.csv' DELIMITER ',' CSV HEADER;"
            ),
            runtime,
        );
    }

    fn get_dialect(&self) -> Dialect {
        Dialect::PostgreSql
    }
}

impl DBConnection for MysqlConnection {
    fn run_query(&mut self, sql: &str, _runtime: &Runtime) -> Vec<Row> {
        let mut conn = self.0.get_conn().unwrap();
        let rows: Vec<mysql::Row> = conn.query(sql).unwrap();
        let mut vec = vec![];
        for row in rows.into_iter() {
            let mut columns = vec![];
            for v in row.unwrap() {
                let value = match v {
                    Value::NULL => "".to_string(),
                    Value::Bytes(v) => String::from_utf8(v).unwrap_or_else(|_| "BLOB".to_string()),
                    Value::Int(v) => v.to_string(),
                    Value::UInt(v) => v.to_string(),
                    Value::Float(v) => v.to_string(),
                    Value::Double(v) => v.to_string(),
                    typ => unimplemented!("mysql type {:?}", typ),
                };
                columns.push(value);
            }
            vec.push(columns);
        }
        vec
    }

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        self.run_query(&format!("LOAD DATA INFILE '/tmp/chinook/{csv_name}.csv' INTO TABLE {csv_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;"), runtime);
    }

    fn get_dialect(&self) -> Dialect {
        Dialect::MySql
    }
}

impl DBConnection for MssqlConnection {
    fn run_query(&mut self, sql: &str, runtime: &Runtime) -> Vec<Row> {
        runtime.block_on(self.query(sql))
    }

    fn import_csv(&mut self, csv_name: &str, runtime: &Runtime) {
        self.run_query(&format!("BULK INSERT {csv_name} FROM '/tmp/chinook/{csv_name}.csv' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');"), runtime);
    }

    fn get_dialect(&self) -> Dialect {
        Dialect::MsSql
    }
}

impl MssqlConnection {
    async fn query(&mut self, sql: &str) -> Vec<Row> {
        let mut stream = self.0.query(sql, &[]).await.unwrap();
        let mut vec = vec![];
        let cols_option = stream.columns().await.unwrap();
        if cols_option.is_none() {
            return vec![];
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
                    ColumnType::Floatn => row
                        .get::<f64, usize>(i)
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "".to_string()),
                    ColumnType::Numericn => row.get::<BigDecimal, usize>(i).unwrap().to_string(),
                    ColumnType::BigVarChar | ColumnType::NVarchar => {
                        String::from(row.get::<&str, usize>(i).unwrap_or(""))
                    }
                    ColumnType::Datetimen => {
                        row.get::<PrimitiveDateTime, usize>(i).unwrap().to_string()
                    }
                    typ => unimplemented!("mssql type {:?}", typ),
                };
                columns.push(value);
            }
            vec.push(columns);
        }

        vec
    }
}
