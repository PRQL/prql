mod connection;

#[cfg(test)]
mod tests {
    use postgres::NoTls;
    use tiberius::{AuthMethod, Client, Config};
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;
    use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

    use prql_compiler::sql::Dialect;
    use prql_compiler::Options;
    use prql_compiler::Target::Sql;

    use crate::connection::*;

    #[ignore]
    #[test]
    fn test_vendors() {
        [5432, 3306, 1433 /*, 50000*/].iter().for_each(|port| {
            if !is_port_open(*port) {
                panic!("No database is listening on port {}", port);
            }
        });
        let runtime = Runtime::new().unwrap();
        let mut duck = DuckDBConnection(duckdb::Connection::open_in_memory().unwrap());
        let mut sqlite = SQLiteConnection(rusqlite::Connection::open_in_memory().unwrap());
        let mut pg = PostgresConnection(
            postgres::Client::connect("host=localhost user=root password=root dbname=dummy", NoTls)
                .unwrap(),
        );
        let mut my =
            MysqlConnection(mysql::Pool::new("mysql://root:root@localhost:3306/dummy").unwrap());
        let mut ms = {
            let mut config = Config::new();
            config.host("127.0.0.1");
            config.port(1433);
            config.trust_cert();
            config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

            let client = runtime.block_on(get_client(config.clone()));

            async fn get_client(config: Config) -> Client<Compat<TcpStream>> {
                let tcp = TcpStream::connect(config.get_addr()).await.unwrap();
                tcp.set_nodelay(true).unwrap();
                Client::connect(config, tcp.compat_write()).await.unwrap()
            }
            MssqlConnection(client)
        };

        let connections: Vec<&mut dyn DBConnection> =
            vec![&mut duck, &mut sqlite, &mut pg, &mut my, &mut ms];

        for con in connections {
            run_tests_for_connection(con, &runtime);
        }
    }

    fn run_tests_for_connection(con: &mut dyn DBConnection, runtime: &Runtime) {
        let setup = include_str!("setup.sql");
        setup
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .for_each(|s| {
                let sql = match con.get_dialect() {
                    Dialect::MsSql => s
                        .replace(" boolean ", " bit ")
                        .replace("TRUE", "1")
                        .replace("FALSE", "0"),
                    Dialect::MySql => s.replace('"', "`"),
                    _ => s.to_string(),
                };
                con.run_query(sql.as_str(), runtime);
            });

        for (prql, expected_rows) in get_test_cases() {
            let options = Options::default().with_target(Sql(Some(con.get_dialect())));
            let sql = prql_compiler::compile(prql.as_str(), &options).unwrap();
            let mut actual_rows = con.run_query(sql.as_str(), runtime);
            replace_booleans(&mut actual_rows);
            println!("{} {:?}", &con.get_dialect(), &actual_rows);
            assert_eq!(
                *expected_rows,
                actual_rows,
                "Rows do not match for {}",
                con.get_dialect()
            );
        }
    }

    // parse test cases from file
    fn get_test_cases() -> Vec<(String, Vec<Row>)> {
        let test_file = include_str!("testcases.txt");
        let tests = test_file
            .split("###")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>();

        tests
            .iter()
            .map(|test| {
                let tests = test
                    .split("---")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<&str>>();
                assert_eq!(tests.len(), 2, "Test is missing ---");

                let rows = tests[1]
                    .lines()
                    .map(|l| {
                        l.split(',')
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect::<Row>()
                    })
                    .collect::<Vec<Row>>();

                (tests[0].to_string(), rows)
            })
            .collect()
    }

    // some sql dialects use 1 and 0 instead of true and false
    fn replace_booleans(rows: &mut Vec<Row>) {
        for row in rows {
            for col in row {
                if col == "true" {
                    *col = "1".to_string();
                } else if col == "false" {
                    *col = "0".to_string();
                }
            }
        }
    }

    fn is_port_open(port: u16) -> bool {
        match std::net::TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                stream.shutdown(std::net::Shutdown::Both).unwrap_or(());
                true
            }
            Err(_) => false,
        }
    }
}
