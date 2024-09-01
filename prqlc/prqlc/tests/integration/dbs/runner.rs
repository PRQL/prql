use std::fs;

use itertools::Itertools;
use regex::Regex;

use super::protocol::DbProtocolHandler;

/// Behavior specific to DBMS
pub trait DbTestRunner: Send {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str);

    fn modify_ddl(&self, sql: String) -> String {
        sql
    }
}

pub struct DuckDbTestRunner;

impl DbTestRunner for DuckDbTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
        protocol
            .query(&format!(
                "COPY {table_name} FROM '{csv_path}' (AUTO_DETECT TRUE);"
            ))
            .unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("FLOAT", "DOUBLE")
    }
}

pub struct SQLiteTestRunner;

impl DbTestRunner for SQLiteTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
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
            protocol.execute(q.as_str()).unwrap();
        }
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("TIMESTAMP", "TEXT") // timestamps in chinook are stores as ISO8601
    }
}

pub struct PostgresTestRunner;

impl DbTestRunner for PostgresTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
        protocol
            .execute(&format!(
                "COPY {table_name} FROM '{csv_path}' DELIMITER ',' CSV HEADER;"
            ))
            .unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("FLOAT", "DOUBLE PRECISION")
    }
}

pub struct GlareDbTestRunner;

impl DbTestRunner for GlareDbTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
        protocol
            .execute(&format!(
                "INSERT INTO {table_name} SELECT * FROM '{csv_path}'"
            ))
            .unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("FLOAT", "DOUBLE PRECISION")
    }
}

pub struct MySqlTestRunner;

impl DbTestRunner for MySqlTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
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
        let query_result = protocol.execute(
            &format!(
                "LOAD DATA INFILE '{}' INTO TABLE {table_name} FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' LINES TERMINATED BY '\n' IGNORE 1 ROWS;",
                &csv_path_binding.parent().unwrap().join(local_new_path.file_name().unwrap()).to_str().unwrap()
            )
        );
        fs::remove_file(&local_new_path).unwrap();
        query_result.unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("TIMESTAMP", "DATETIME")
    }
}

pub struct ClickHouseTestRunner;

impl DbTestRunner for ClickHouseTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
        protocol
            .execute(&format!(
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
    }
}

pub struct MsSqlTestRunner;

impl DbTestRunner for MsSqlTestRunner {
    fn import_csv(&self, protocol: &mut dyn DbProtocolHandler, csv_path: &str, table_name: &str) {
        protocol.execute(&format!("BULK INSERT {table_name} FROM '{csv_path}' WITH (FIRSTROW = 2, FIELDTERMINATOR = ',', ROWTERMINATOR = '\n', TABLOCK, FORMAT = 'CSV', CODEPAGE = 'RAW');")).unwrap();
    }

    fn modify_ddl(&self, sql: String) -> String {
        sql.replace("TIMESTAMP", "DATETIME")
            .replace("REAL", "FLOAT(53)")
            .replace(" AS TEXT", " AS VARCHAR")
    }
}
