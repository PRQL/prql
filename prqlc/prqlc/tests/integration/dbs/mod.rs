#![cfg(not(target_family = "wasm"))]
#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]
/// This has been refactored many times by many people — to the extent that it's
/// somewhat of a tradition at PRQL.
///
/// As of the most recent refactor in 2024-08, I think it's in good shape.
///
/// The one remaining question is whether the `*TestRunner::new` implementations
/// should instead be on the `Protocol`s; the `url` parameter exists for
/// some-but-not-all of the Protocols; given it's a trait implementation the
/// signatures need to be the same. So we could move the `url` parameter into
/// the protocols themselves (it seems somewhat arbitrary to have some things
/// like the port within the TestRunner, but the url passed into the TestRunner).
/// Note that we can't push the `import_csv` down to the Protocol, because
/// different DBs use the same protocol but different CSV import methods.
mod protocol;
mod runner;

use connector_arrow::arrow;
use prqlc::sql::SupportLevel;
use regex::Regex;

use self::runner::DbTestRunner;

pub(crate) fn runners() -> &'static std::sync::Mutex<Vec<Box<dyn DbTestRunner>>> {
    static RUNNERS: std::sync::OnceLock<std::sync::Mutex<Vec<Box<dyn DbTestRunner>>>> =
        std::sync::OnceLock::new();
    RUNNERS.get_or_init(|| {
        let mut runners = vec![];

        let local_runners: Vec<Box<dyn DbTestRunner>> = vec![
            Box::new(runner::SQLiteTestRunner::new(
                "tests/integration/data/chinook".to_string(),
            )),
            Box::new(runner::DuckDbTestRunner::new(
                "tests/integration/data/chinook".to_string(),
            )),
        ];
        runners.extend(local_runners);

        #[cfg(feature = "test-dbs-external")]
        {
            let external_runners: Vec<Box<dyn DbTestRunner>> = vec![
                Box::new(runner::PostgresTestRunner::new(
                    "host=localhost user=root password=root dbname=dummy",
                    "/tmp/chinook".to_string(),
                )),
                Box::new(runner::MySqlTestRunner::new(
                    "mysql://root:root@localhost:3306/dummy",
                    "/tmp/chinook".to_string(),
                )),
                // TODO: https://github.com/ClickHouse/ClickHouse/issues/69131
                // Box::new(runner::ClickHouseTestRunner::new(
                //     "mysql://default:@localhost:9004/dummy",
                //     "chinook".to_string(),
                // )),
                Box::new(runner::GlareDbTestRunner::new(
                    "host=localhost user=glaredb dbname=glaredb port=6543",
                    "/tmp/chinook".to_string(),
                )),
                Box::new(runner::MsSqlTestRunner::new("/tmp/chinook".to_string())),
            ];
            runners.extend(external_runners);
        }
        std::sync::Mutex::new(
            runners
                .into_iter()
                .filter(|cfg| {
                    matches!(
                        cfg.dialect().support_level(),
                        SupportLevel::Supported | SupportLevel::Unsupported
                    )
                })
                .map(|mut runner| {
                    runner.setup();
                    runner
                })
                .collect(),
        )
    })
}

/// Converts arrow::RecordBatch into ad-hoc CSV
pub(crate) fn batch_to_csv(batch: arrow::record_batch::RecordBatch) -> String {
    let mut res = String::with_capacity((batch.num_rows() + 1) * batch.num_columns() * 20);

    // convert each column to string
    let mut arrays = Vec::with_capacity(batch.num_columns());
    for col_i in 0..batch.num_columns() {
        let mut array = batch.columns().get(col_i).unwrap().clone();
        if *array.data_type() == arrow::datatypes::DataType::Boolean {
            array = arrow::compute::cast(&array, &arrow::datatypes::DataType::UInt8).unwrap();
        }
        let array = arrow::compute::cast(&array, &arrow::datatypes::DataType::Utf8).unwrap();
        let array = arrow::array::AsArray::as_string::<i32>(&array).clone();
        arrays.push(array);
    }

    let re = Regex::new(r"^-?\d+\.\d*0+$").unwrap();
    for row_i in 0..batch.num_rows() {
        for (i, col) in arrays.iter().enumerate() {
            let mut value = col.value(row_i);

            // HACK: trim trailing 0
            if re.is_match(value) {
                value = value.trim_end_matches('0').trim_end_matches('.');
            }
            res.push_str(value);
            if i < batch.num_columns() - 1 {
                res.push(',');
            } else {
                res.push('\n');
            }
        }
    }

    res.shrink_to_fit();
    res
}
