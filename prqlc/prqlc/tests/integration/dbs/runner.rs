use anyhow::Result;
use connector_arrow::arrow::record_batch::RecordBatch;
use glob::glob;
use prqlc::sql::Dialect;

use super::protocol::DbProtocol;

pub(crate) trait DbTestRunner: Send {
    fn dialect(&self) -> Dialect;
    fn data_file_root(&self) -> &str;
    fn import_csv(&mut self, csv_path: &str, table_name: &str);
    fn modify_ddl(&self, sql: String) -> String;
    fn query(&mut self, sql: &str) -> Result<RecordBatch>;
    fn protocol(&mut self) -> &mut dyn DbProtocol;
    fn setup(&mut self) {
        let schema = include_str!("../data/chinook/schema.sql");
        let statements: Vec<String> = schema
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| self.modify_ddl(s.to_string()))
            .collect();

        for statement in statements {
            self.protocol().execute(&statement).unwrap();
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
    protocol: Box<dyn DbProtocol>,
    data_file_root: String,
}

impl DuckDbTestRunner {
    pub(crate) fn new(data_file_root: String) -> Self {
        let conn = ::duckdb::Connection::open_in_memory().unwrap();
        let conn_ar = connector_arrow::duckdb::DuckDBConnection::new(conn);
        Self {
            protocol: Box::new(conn_ar),
            data_file_root,
        }
    }
}

impl DbTestRunner for DuckDbTestRunner {
    fn dialect(&self) -> Dialect {
        Dialect::DuckDb
    }

    fn protocol(&mut self) -> &mut dyn DbProtocol {
        self.protocol.as_mut()
    }

    fn data_file_root(&self) -> &str {
        &self.data_file_root
    }

    fn import_csv(&mut self, csv_path: &str, table_name: &str) {
        self.protocol
            .execute(&format!(
                "COPY {table_name} FROM '{csv_path}' (AUTO_DETECT TRUE);"
            ))
            .unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("FLOAT", "DOUBLE")
    }

    fn query(&mut self, sql: &str) -> Result<RecordBatch> {
        self.protocol.query(sql)
    }
}

pub(crate) struct SQLiteTestRunner {
    protocol: Box<dyn DbProtocol>,
    data_file_root: String,
}

impl SQLiteTestRunner {
    pub(crate) fn new(data_file_root: String) -> Self {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let conn_ar = connector_arrow::sqlite::SQLiteConnection::new(conn);
        Self {
            protocol: Box::new(conn_ar),
            data_file_root,
        }
    }
}

impl DbTestRunner for SQLiteTestRunner {
    fn dialect(&self) -> Dialect {
        Dialect::SQLite
    }

    fn protocol(&mut self) -> &mut dyn DbProtocol {
        self.protocol.as_mut()
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
                headers.join(","),
                r.iter()
                    .map(|s| if s.is_empty() {
                        "null".to_string()
                    } else {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            );
            self.protocol.execute(&q).unwrap();
        }
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("TIMESTAMP", "TEXT") // timestamps in chinook are stored as ISO8601
    }

    fn query(&mut self, sql: &str) -> Result<RecordBatch> {
        self.protocol.query(sql)
    }
}

#[cfg(feature = "test-dbs-external")]
pub(crate) use self::external::*;

#[cfg(feature = "test-dbs-external")]
pub(crate) mod external {
    use super::*;
    use std::fs;

    pub(crate) struct PostgresTestRunner {
        protocol: Box<dyn DbProtocol>,
        data_file_root: String,
    }

    impl PostgresTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            use connector_arrow::postgres::PostgresConnection;
            let client = ::postgres::Client::connect(url, ::postgres::NoTls).unwrap();
            Self {
                protocol: Box::new(PostgresConnection::new(client)),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for PostgresTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::Postgres
        }

        fn protocol(&mut self) -> &mut dyn DbProtocol {
            self.protocol.as_mut()
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .execute(&format!(
                    "COPY {table_name} FROM '{csv_path}' DELIMITER ',' CSV HEADER;"
                ))
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("FLOAT", "DOUBLE PRECISION")
        }

        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct MySqlTestRunner {
        protocol: Box<dyn DbProtocol>,
        data_file_root: String,
    }

    impl MySqlTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            let conn = ::mysql::Conn::new(url)
                .unwrap_or_else(|e| panic!("Failed to connect to {}:\n{}", url, e));
            Self {
                protocol: Box::new(
                    connector_arrow::mysql::MySQLConnection::<::mysql::Conn>::new(conn),
                ),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for MySqlTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::MySql
        }

        fn protocol(&mut self) -> &mut dyn DbProtocol {
            self.protocol.as_mut()
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            // MySQL-specific CSV import logic
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
            self.protocol.execute(
                &format!(
                    "LOAD DATA INFILE '{}' INTO TABLE {table_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;",
                    &csv_path_binding.parent().unwrap().join(local_new_path.file_name().unwrap()).to_str().unwrap()
                )
            ).unwrap();
            fs::remove_file(&local_new_path).unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("TIMESTAMP", "DATETIME")
        }

        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct ClickHouseTestRunner {
        protocol: Box<dyn DbProtocol>,
        data_file_root: String,
    }

    impl ClickHouseTestRunner {
        #[allow(dead_code)]
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            Self {
                protocol: Box::new(
                    connector_arrow::mysql::MySQLConnection::<::mysql::Conn>::new(
                        ::mysql::Conn::new(url)
                            .unwrap_or_else(|e| panic!("Failed to connect to {}:\n{}", url, e)),
                    ),
                ),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for ClickHouseTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::ClickHouse
        }

        fn protocol(&mut self) -> &mut dyn DbProtocol {
            self.protocol.as_mut()
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .execute(&format!(
                    "INSERT INTO {table_name} SELECT * FROM file('{csv_path}')"
                ))
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            use regex::Regex;
            let re = Regex::new(r"(?s)\)$").unwrap();
            re.replace(&sql, r") ENGINE = Memory")
                .replace("TIMESTAMP", "DATETIME64")
                .replace("FLOAT", "DOUBLE")
                .replace("VARCHAR(255)", "Nullable(String)")
                .to_string()
        }

        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct MsSqlTestRunner {
        protocol: Box<dyn DbProtocol>,
        data_file_root: String,
    }

    impl MsSqlTestRunner {
        pub(crate) fn new(data_file_root: String) -> Self {
            use std::sync::Arc;
            use tokio_util::compat::TokioAsyncWriteCompatExt;

            let mut config = tiberius::Config::new();
            config.host("localhost");
            config.port(1433);
            config.trust_cert();
            config.authentication(tiberius::AuthMethod::sql_server("sa", "Wordpass123##"));

            let rt = Arc::new(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
            );

            let client = rt
                .block_on(async {
                    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
                    tcp.set_nodelay(true).unwrap();
                    tiberius::Client::connect(config, tcp.compat_write()).await
                })
                .unwrap();

            Self {
                protocol: Box::new(connector_arrow::tiberius::TiberiusConnection::new(
                    rt, client,
                )),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for MsSqlTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::MsSql
        }

        fn protocol(&mut self) -> &mut dyn DbProtocol {
            self.protocol.as_mut()
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol.execute(&format!(
                "BULK INSERT {table_name} FROM '{csv_path}' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');"
            )).unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("TIMESTAMP", "DATETIME")
                .replace("FLOAT", "FLOAT(53)")
                .replace(" AS TEXT", " AS VARCHAR")
        }

        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            self.protocol.query(sql)
        }
    }

    pub(crate) struct GlareDbTestRunner {
        protocol: Box<dyn DbProtocol>,
        data_file_root: String,
    }

    impl GlareDbTestRunner {
        pub(crate) fn new(url: &str, data_file_root: String) -> Self {
            use connector_arrow::postgres::PostgresConnection;
            let client = ::postgres::Client::connect(url, ::postgres::NoTls).unwrap();
            Self {
                protocol: Box::new(PostgresConnection::new(client)),
                data_file_root,
            }
        }
    }

    impl DbTestRunner for GlareDbTestRunner {
        fn dialect(&self) -> Dialect {
            Dialect::GlareDb
        }

        fn protocol(&mut self) -> &mut dyn DbProtocol {
            self.protocol.as_mut()
        }

        fn data_file_root(&self) -> &str {
            &self.data_file_root
        }

        fn import_csv(&mut self, csv_path: &str, table_name: &str) {
            self.protocol
                .execute(&format!(
                    "INSERT INTO {table_name} SELECT * FROM '{csv_path}'"
                ))
                .unwrap();
        }

        fn modify_ddl(&self, sql: String) -> String {
            sql.replace("FLOAT", "DOUBLE PRECISION")
        }

        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            self.protocol.query(sql)
        }
    }
}
