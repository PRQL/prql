#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use chrono::{DateTime, Utc};
    use insta::{assert_snapshot, glob};
    use pg_bigdecimal::PgNumeric;
    use postgres::NoTls;
    use postgres::types::{Type};

    use prql_compiler::{sql::Dialect};

    type Row = Vec<String>;

    const TEST_CASES: Vec<(&str, Vec<Row>)> = vec![];

    #[test]
    fn test_vendor() {
        let mut pg = PostgresConnection(postgres::Client::connect("host=localhost user=root password=root dbname=dummy", NoTls).unwrap());
        let mut duck = DuckDBConnection(duckdb::Connection::open_in_memory().unwrap());
        let mut sqlite = SQLiteConnection(rusqlite::Connection::open_in_memory().unwrap());
        let mut connections: Vec<&mut dyn DBConnection> = vec![];
        connections.push(&mut pg);
        connections.push(&mut duck);
        connections.push(&mut sqlite);

        for con in connections {
            let e = con.run_query("select 1=1 bo,1+1 i2,200000004400+1 i4,'tte' te,0.1+0.2 f;");
            println!("{:?}", e);
        }
        panic!("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ {:?}", 6);
    }

    trait DBConnection {
        fn run_query(&mut self, sql: &str) -> Vec<Row>;

        fn get_dialect(&self) -> Dialect;
    }

    struct DuckDBConnection(duckdb::Connection);

    struct SQLiteConnection(rusqlite::Connection);

    struct PostgresConnection(postgres::Client);

    impl DBConnection for DuckDBConnection {
        fn run_query(&mut self, sql: &str) -> Vec<Row> {
            let mut statement = self.0.prepare(sql).unwrap();
            let mut rows = statement.query([]).unwrap();
            let mut vec = vec![];
            while let Ok(Some(row)) = rows.next() {
                let mut columns = vec![];
                for i in 0.. {
                    let v_ref = match row.get_ref(i) {
                        Ok(v) => { v }
                        Err(_) => { break; }
                    };
                    let value = match v_ref {
                        duckdb::types::ValueRef::Null => { "".to_string() }
                        duckdb::types::ValueRef::Boolean(v) => { v.to_string() }
                        duckdb::types::ValueRef::TinyInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::SmallInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::Int(v) => { v.to_string() }
                        duckdb::types::ValueRef::BigInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::HugeInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::UTinyInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::USmallInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::UInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::UBigInt(v) => { v.to_string() }
                        duckdb::types::ValueRef::Float(v) => { v.to_string() }
                        duckdb::types::ValueRef::Double(v) => { v.to_string() }
                        duckdb::types::ValueRef::Decimal(v) => { v.to_string() }
                        duckdb::types::ValueRef::Timestamp(u, v) => { format!("{} {:?}", v, u) }
                        duckdb::types::ValueRef::Text(v) => { String::from_utf8(v.to_vec()).unwrap() }
                        duckdb::types::ValueRef::Blob(v) => { String::from_utf8(v.to_vec()).unwrap() }
                        duckdb::types::ValueRef::Date32(v) => { v.to_string() }
                        duckdb::types::ValueRef::Time64(u, v) => { format!("{} {:?}", v, u) }
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
        fn run_query(&mut self, sql: &str) -> Vec<Row> {
            let mut statement = self.0.prepare(sql).unwrap();
            let mut rows = statement.query([]).unwrap();
            let mut vec = vec![];
            while let Ok(Some(row)) = rows.next() {
                let mut columns = vec![];
                for i in 0.. {
                    let v_ref = match row.get_ref(i) {
                        Ok(v) => { v }
                        Err(_) => { break; }
                    };
                    let value = match v_ref {
                        rusqlite::types::ValueRef::Null => { "".to_string() }
                        rusqlite::types::ValueRef::Integer(v) => { v.to_string() }
                        rusqlite::types::ValueRef::Real(v) => { v.to_string() }
                        rusqlite::types::ValueRef::Text(v) => { String::from_utf8(v.to_vec()).unwrap() }
                        rusqlite::types::ValueRef::Blob(v) => { String::from_utf8(v.to_vec()).unwrap() }
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
        fn run_query(&mut self, sql: &str) -> Vec<Row> {
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
                        &Type::TEXT | &Type::VARCHAR => row.get::<usize, String>(i),
                        &Type::JSON | &Type::JSONB => row.get::<usize, String>(i),
                        &Type::FLOAT4 => (row.get::<usize, f32>(i)).to_string(),
                        &Type::FLOAT8 => (row.get::<usize, f64>(i)).to_string(),
                        &Type::NUMERIC => row.get::<usize, PgNumeric>(i).n.unwrap().to_string(),
                        &Type::TIMESTAMPTZ | &Type::TIMESTAMP => {
                            let time = row.get::<usize, SystemTime>(i);
                            let date_time: DateTime<Utc> = time.into();
                            date_time.to_rfc3339()
                        }
                        t => unimplemented!("postgres type {t}"),
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
}