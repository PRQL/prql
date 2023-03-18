#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use chrono::{DateTime, Utc};
    use mysql::prelude::Queryable;
    use mysql::Value;
    use pg_bigdecimal::PgNumeric;
    use postgres::types::Type;
    use postgres::NoTls;
    use tiberius::numeric::BigDecimal;
    use tiberius::*;
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;
    use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

    use prql_compiler::sql::Dialect;
    use prql_compiler::Options;
    use prql_compiler::Target::Sql;

    type Row = Vec<String>;

    #[test]
    fn test_vendor() {
        let test_cases: Vec<(&str, Vec<Row>)> = vec![(
            "from c=customers
            join ca=cars [ca.customer==c.id]
            filter ca.name=='Bugatti'
            select c.name",
            vec![vec!["Tony Stark".to_string()]],
        )];

        let mut duck = DuckDBConnection(duckdb::Connection::open_in_memory().unwrap());
        let mut sqlite = SQLiteConnection(rusqlite::Connection::open_in_memory().unwrap());
        let mut pg = PostgresConnection(
            postgres::Client::connect("host=localhost user=root password=root dbname=dummy", NoTls)
                .unwrap(),
        );
        let mut my =
            MysqlConnection(mysql::Pool::new("mysql://root:root@localhost:3306/dummy").unwrap());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut ms = {
            let mut config = Config::new();
            config.host("127.0.0.1");
            config.port(1433);
            config.trust_cert();
            config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

            let client = rt.block_on(get_client(config.clone()));

            async fn get_client(config: Config) -> Client<Compat<TcpStream>> {
                let tcp = TcpStream::connect(config.get_addr()).await.unwrap();
                tcp.set_nodelay(true).unwrap();
                tiberius::Client::connect(config, tcp.compat_write())
                    .await
                    .unwrap()
            }
            MssqlConnection(client)
        };

        let connections: Vec<&mut dyn DBConnection> =
            vec![&mut duck, &mut sqlite, &mut pg, &mut my, &mut ms];

        for con in connections {
            let setup = include_str!("setup.sql");
            setup
                .split(";")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .for_each(|s| {
                    let sql = match con.get_dialect() {
                        Dialect::MsSql => s
                            .replace(" boolean ", " bit ")
                            .replace("TRUE", "1")
                            .replace("FALSE", "0"),
                        _ => s.to_string(),
                    };
                    con.run_query(sql.as_str(), Some(&rt));
                });

            for (prql, expected_rows) in test_cases.iter() {
                let options = Options::default().with_target(Sql(Some(con.get_dialect())));
                let sql = prql_compiler::compile(prql, &options).unwrap();
                let mut actual_rows = con.run_query(sql.as_str(), Some(&rt));
                replace_booleans(&mut actual_rows);
                assert_eq!(
                    *expected_rows,
                    actual_rows,
                    "Rows do not match for {}",
                    con.get_dialect()
                );
            }
        }
    }

    fn replace_booleans(rows: &mut Vec<Row>) {
        for row in rows {
            for col in row {
                if col == &"true" {
                    *col = "1".to_string();
                } else if col == &"false" {
                    *col = "0".to_string();
                }
            }
        }
    }

    trait DBConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row>;

        fn get_dialect(&self) -> Dialect;
    }

    struct DuckDBConnection(duckdb::Connection);

    struct SQLiteConnection(rusqlite::Connection);

    struct PostgresConnection(postgres::Client);

    struct MysqlConnection(mysql::Pool);

    struct MssqlConnection(tiberius::Client<Compat<TcpStream>>);

    impl DBConnection for DuckDBConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row> {
            let mut statement = self.0.prepare(sql).unwrap();
            let mut rows = statement.query([]).unwrap();
            let mut vec = vec![];
            while let Ok(Some(row)) = rows.next() {
                let mut columns = vec![];
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

        fn get_dialect(&self) -> Dialect {
            Dialect::DuckDb
        }
    }

    impl DBConnection for SQLiteConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row> {
            let mut statement = self.0.prepare(sql).unwrap();
            let mut rows = statement.query([]).unwrap();
            let mut vec = vec![];
            while let Ok(Some(row)) = rows.next() {
                let mut columns = vec![];
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
            vec
        }

        fn get_dialect(&self) -> Dialect {
            Dialect::SQLite
        }
    }

    impl DBConnection for PostgresConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row> {
            let rows = self.0.query(sql, &[]).unwrap();
            let mut vec = vec![];
            for row in rows.into_iter() {
                let mut columns = vec![];
                for i in 0..row.len() {
                    let col = &(*row.columns())[i];
                    let value = match col.type_() {
                        &Type::BOOL => (row.get::<usize, bool>(i)).to_string(),
                        &Type::INT4 => (row.get::<usize, i32>(i)).to_string(),
                        &Type::INT8 => (row.get::<usize, i64>(i)).to_string(),
                        &Type::TEXT => {
                            match row.try_get::<usize, String>(i) {
                                Ok(v) => v,
                                // handle null
                                Err(_) => "".to_string(),
                            }
                        }
                        &Type::VARCHAR | &Type::JSON | &Type::JSONB => row.get::<usize, String>(i),
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

        fn get_dialect(&self) -> Dialect {
            Dialect::PostgreSql
        }
    }

    impl DBConnection for MysqlConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row> {
            let mut conn = self.0.get_conn().unwrap();
            let rows: Vec<mysql::Row> = conn.query(sql).unwrap();
            let mut vec = vec![];
            for row in rows.into_iter() {
                let mut columns = vec![];
                for v in row.unwrap() {
                    let value = match v {
                        Value::NULL => "".to_string(),
                        Value::Bytes(v) => String::from_utf8(v).unwrap_or("BLOB".to_string()),
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

        fn get_dialect(&self) -> Dialect {
            Dialect::MySql
        }
    }

    impl DBConnection for MssqlConnection {
        fn run_query(&mut self, sql: &str, rt: Option<&Runtime>) -> Vec<Row> {
            let runtime = rt.unwrap();
            runtime.block_on(self.query(sql))
        }

        fn get_dialect(&self) -> Dialect {
            Dialect::MsSql
        }
    }

    impl MssqlConnection {
        async fn query(&mut self, sql: &str) -> Vec<Row> {
            let mut stream = self.0.query(sql, &[]).await.unwrap();
            let mut vec = vec![];
            let cols_option = (&mut stream).columns().await.unwrap();
            if cols_option.is_none() {
                return vec![];
            }
            let cols = cols_option.unwrap().to_vec();
            for row in stream.into_first_result().await.unwrap() {
                let mut columns = vec![];
                for i in 0..row.len() {
                    let col = &cols[i];
                    let value = match col.column_type() {
                        ColumnType::Null => "".to_string(),
                        ColumnType::Bit => String::from(row.get::<&str, usize>(i).unwrap()),
                        ColumnType::Intn => row
                            .get::<i32, usize>(i)
                            .map(|i| i.to_string())
                            .unwrap_or("".to_string()),
                        ColumnType::Numericn => {
                            row.get::<BigDecimal, usize>(i).unwrap().to_string()
                        }
                        ColumnType::BigVarChar => String::from(row.get::<&str, usize>(i).unwrap()),
                        typ => unimplemented!("mssql type {:?}", typ),
                    };
                    columns.push(value);
                }
                vec.push(columns);
            }

            vec
        }
    }
}
