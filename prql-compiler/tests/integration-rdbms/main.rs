#![cfg(not(any(target_family = "windows", target_family = "wasm")))]

mod connection;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::{env, fs};

    use insta::{assert_snapshot, glob};
    use postgres::NoTls;
    use tiberius::{AuthMethod, Client, Config};
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;
    use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

    use prql_compiler::sql::Dialect;
    use prql_compiler::Options;
    use prql_compiler::Target::Sql;

    use crate::connection::*;

    #[test]
    fn test_rdbms() {
        for port in [5432u16, 3306, 1433 /*, 50000*/] {
            // test is skipped locally when DB is not listening
            // in CI it fails
            if !is_port_open(port) {
                match env::var("CI") {
                    Ok(v) if &v == "true" => {
                        // CI
                        panic!("No database is listening on port {}", port);
                    }
                    _ => {
                        // locally
                        return;
                    }
                }
            }
        }
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

        let mut connections: Vec<&mut dyn DBConnection> =
            vec![&mut duck, &mut sqlite, &mut pg, &mut my, &mut ms];

        for con in &mut connections {
            setup_connection(*con, &runtime);
        }

        // for each of the queries
        glob!("..", "integration/queries/**/*.prql", |path| {
            let test_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();

            // read
            let prql = fs::read_to_string(path).unwrap();

            if prql.contains("skip_test") {
                return;
            }

            let mut results = BTreeMap::new();
            for con in &mut connections {
                let vendor = con.get_dialect().to_string().to_lowercase();
                if prql.contains(format!("skip_{}", vendor).as_str()) {
                    continue;
                }
                results.insert(vendor, run_query(*con, prql.as_str(), &runtime));
            }

            let first_result = match results.iter().next() {
                Some(v) => v,
                None => return,
            };
            for (k, v) in results.iter().skip(1) {
                pretty_assertions::assert_eq!(
                    *first_result.1,
                    *v,
                    "{} == {}: {test_name}",
                    first_result.0,
                    k
                );
            }

            assert_snapshot!(format!("{:?}", first_result.1));
        });
    }

    fn setup_connection(con: &mut dyn DBConnection, runtime: &Runtime) {
        let setup = include_str!("../integration/data/chinook/schema.sql");
        setup
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .for_each(|s| {
                let sql = match con.get_dialect() {
                    Dialect::MsSql => s.replace("TIMESTAMP", "DATETIME"),
                    Dialect::MySql => s.replace('"', "`").replace("TIMESTAMP", "DATETIME"),
                    _ => s.to_string(),
                };
                con.run_query(sql.as_str(), runtime);
            });
        let tables = [
            "invoices",
            "customers",
            "employees",
            "tracks",
            "albums",
            "genres",
            "playlist_track",
            "playlists",
            "media_types",
            "artists",
            "invoice_items",
        ];
        for table in tables {
            con.import_csv(table, runtime);
        }
    }

    fn run_query(con: &mut dyn DBConnection, prql: &str, runtime: &Runtime) -> Vec<Row> {
        let options = Options::default().with_target(Sql(Some(con.get_dialect())));
        let sql = prql_compiler::compile(prql, &options).unwrap();
        let mut actual_rows = con.run_query(sql.as_str(), runtime);
        replace_booleans(&mut actual_rows);
        actual_rows
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
