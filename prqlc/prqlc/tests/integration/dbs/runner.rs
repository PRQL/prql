use anyhow::Result;
use glob::glob;
use itertools::Itertools;

use super::{protocol::DbProtocol, Row};
use prqlc::sql::Dialect;

pub(crate) trait DbTestRunner: Send {
    fn dialect(&self) -> Dialect;
    fn data_file_root(&self) -> &str;
    fn import_csv(&mut self, csv_path: &str, table_name: &str);
    fn modify_ddl(&self, sql: String) -> String;
    fn query(&mut self, sql: &str) -> Result<Vec<Row>>;
    fn setup(&mut self) {
        let schema = include_str!("../data/chinook/schema.sql");
        let statements: Vec<String> = schema
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| self.modify_ddl(s.to_string()))
            .map(|s| s.to_string())
            .collect();

        for statement in statements {
            self.query(&statement).unwrap();
        }

        for file_path in glob("tests/integration/data/chinook/*.csv").unwrap() {
            let file_path = file_path.unwrap();
            let stem = file_path.file_stem().unwrap().to_str().unwrap();
            let path = format!("{}/{}.csv", self.data_file_root(), stem);
            self.import_csv(&path, stem);
        }
    }
}

pub(crate) struct DuckDbTestRunner {
    protocol: duckdb::Connection,
    data_file_root: String,
}

impl DuckDbTestRunner {
    pub(crate) fn new(data_file_root: String) -> Self {
        Self {
            protocol: duckdb::Connection::open_in_memory().unwrap(),
            data_file_root,
        }
    }
}

impl DbTestRunner for DuckDbTestRunner {
    fn dialect(&self) -> Dialect {
        Dialect::DuckDb
    }

    fn data_file_root(&self) -> &str {
        &self.data_file_root
    }

    fn import_csv(&mut self, csv_path: &str, table_name: &str) {
        self.protocol
            .query(&format!(
                "COPY {table_name} FROM '{csv_path}' (AUTO_DETECT TRUE);"
            ))
            .unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("REAL", "DOUBLE")
    }

    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        self.protocol.query(sql)
    }
}

pub(crate) struct SQLiteTestRunner {
    protocol: rusqlite::Connection,
    data_file_root: String,
}

impl SQLiteTestRunner {
    pub(crate) fn new(data_file_root: String) -> Self {
        Self {
            protocol: rusqlite::Connection::open_in_memory().unwrap(),
            data_file_root,
        }
    }
}

impl DbTestRunner for SQLiteTestRunner {
    fn dialect(&self) -> Dialect {
        Dialect::SQLite
    }

    fn data_file_root(&self) -> &str {
        &self.data_file_root
    }

    fn import_csv(&mut self, csv_path: &str, table_name: &str) {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(csv_path)
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
                "INSERT INTO {table_name} ({}) VALUES ({})",
                headers.iter().join(","),
                r.iter()
                    .map(|s| if s.is_empty() {
                        "null".to_string()
                    } else {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    })
                    .join(",")
            );
            self.query(q.as_str()).unwrap();
        }
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("TIMESTAMP", "TEXT") // timestamps in chinook are stores as ISO8601
    }

    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        self.protocol.query(sql)
    }
}

#[cfg(feature = "test-dbs-external")]
pub(crate) use external::*;

#[cfg(feature = "test-dbs-external")]
mod external {

    use regex::Regex;
    use std::{fs, sync::OnceLock};
    use tokio_util::compat::TokioAsyncWriteCompatExt;

    use prqlc::sql::Dialect;

    use super::*;

    pub(crate) struct PostgresTestRunner {
        protocol: postgres::Client,
        data_file_root: String,
    }

    impl PostgresTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            Self {
                protocol: postgres::Client::connect(url, postgres::NoTls).unwrap(),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for PostgresTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::Postgres
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .execute(
                    &format!("COPY {table_name} FROM '{csv_path}' DELIMITER ',' CSV HEADER;"),
                    &[],
                )
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("REAL", "DOUBLE PRECISION")
        }

        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            DbProtocol::query(&mut self.protocol, sql)
        }
    }

    pub(crate) struct GlareDbTestRunner {
        protocol: postgres::Client,
        data_file_root: String,
    }

    impl GlareDbTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            Self {
                protocol: postgres::Client::connect(url, postgres::NoTls).unwrap(),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for GlareDbTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::GlareDb
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .execute(
                    &format!("INSERT INTO {table_name} SELECT * FROM '{csv_path}'"),
                    &[],
                )
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("REAL", "DOUBLE PRECISION")
        }

        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            DbProtocol::query(&mut self.protocol, sql)
        }
    }

    pub(crate) struct MySqlTestRunner {
        protocol: mysql::Pool,
        data_file_root: String,
    }

    impl MySqlTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            Self {
                protocol: mysql::Pool::new(url).unwrap(),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for MySqlTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::MySql
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            // hacky hack for MySQL
            // MySQL needs a special character in csv that means NULL (https://stackoverflow.com/a/2675493)
            // 1. read the csv
            // 2. create a copy with the special character
            // 3. import the data and remove the copy

            let csv_path_binding = std::path::PathBuf::from(csv_path);
            let local_csv_path = format!(
                "tests/integration/data/chinook/{}",
                csv_path_binding.file_name().unwrap().to_str().unwrap()
            );
            let local_old_path = std::path::PathBuf::from(local_csv_path);
            let mut local_new_path = local_old_path.clone();
            local_new_path.pop();
            local_new_path.push(format!("{table_name}.my.csv").as_str());
            let mut file_content = fs::read_to_string(local_old_path).unwrap();
            file_content = file_content.replace(",,", ",\\N,").replace(",\n", ",\\N\n");
            fs::write(&local_new_path, file_content).unwrap();
            let query_result = self.protocol.query(
            &format!(
                "LOAD DATA INFILE '{}' INTO TABLE {table_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;",
                &csv_path_binding.parent().unwrap().join(local_new_path.file_name().unwrap()).to_str().unwrap()
            ));
            fs::remove_file(&local_new_path).unwrap();
            query_result.unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("TIMESTAMP", "DATETIME")
        }

        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct ClickHouseTestRunner {
        protocol: mysql::Pool,
        data_file_root: String,
    }

    impl ClickHouseTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            Self {
                protocol: mysql::Pool::new(url).unwrap(),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for ClickHouseTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::ClickHouse
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .query(&format!(
                    "INSERT INTO {table_name} SELECT * FROM file('{csv_path}')"
                ))
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            let re = Regex::new(r"(?s)\)$").unwrap();
            re.replace(&sql, r") ENGINE = Memory")
                .replace("TIMESTAMP", "DATETIME64")
                .replace("REAL", "DOUBLE")
                .replace("VARCHAR(255)", "Nullable(String)")
                .to_string()
        }

        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct MsSqlTestRunner {
        protocol: tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>,
        data_file_root: String,
    }

    impl MsSqlTestRunner {
        pub(crate) fn new(data_file_root: String) -> Self {
            let mut config = tiberius::Config::new();
            config.host("localhost");
            config.port(1433);
            config.trust_cert();
            config.authentication(tiberius::AuthMethod::sql_server("sa", "Wordpass123##"));

            let protocol = Self::runtime().block_on(async {
                let tcp = tokio::net::TcpStream::connect(config.get_addr())
                    .await
                    .unwrap();
                tcp.set_nodelay(true).unwrap();
                tiberius::Client::connect(config, tcp.compat_write())
                    .await
                    .unwrap()
            });

            Self {
                protocol,
                data_file_root,
            }
        }
        fn runtime() -> &'static tokio::runtime::Runtime {
            static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            RUNTIME.get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
            })
        }
    }

    impl DbTestRunner for MsSqlTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::MsSql
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            Self::runtime().block_on(async {
            self.protocol
                .execute(
                    &format!(
                        "BULK INSERT {table_name} FROM '{csv_path}' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');"
                    ),
                    &[],
                )
                .await
                .unwrap();
        });
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("TIMESTAMP", "DATETIME")
                .replace("REAL", "FLOAT(53)")
                .replace(" AS TEXT", " AS VARCHAR")
        }

        fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
            DbProtocol::query(&mut self.protocol, sql)
        }
    }
}
