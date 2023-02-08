//! Feature map for SQL dialects.
//!
//! The general principle with is to strive to target only the generic (i.e. default) dialect.
//!
//! This means that we prioritize common dialects and old dialect versions, because such
//! implementations would also be supported by newer versions.
//!
//! Dialect-specifics should be added only if:
//! - the generic dialect is not supported (i.e. LIMIT is not supported in MS SQL),
//! - dialect-specific impl is more performant than generic impl.
//!
//! As a consequence, generated SQL may be verbose, since it will avoid newer or less adopted SQL
//! constructs. The upside is much less complex translator.

use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use strum::{EnumMessage, IntoEnumIterator};

/// SQL dialect.
///
/// This is only changes the output for a relatively small subset of features.
///
/// If something does not work in a specific dialect, please raise in a
/// GitHub issue.
// Make sure to update Python bindings, JS bindings & docs in the book.
#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumMessage,
    strum::EnumString,
)]
pub enum Dialect {
    #[strum(serialize = "ansi")]
    Ansi,
    #[strum(serialize = "bigquery")]
    BigQuery,
    #[strum(serialize = "clickhouse")]
    ClickHouse,
    #[strum(serialize = "duckdb")]
    DuckDb,
    #[strum(serialize = "generic")]
    Generic,
    #[strum(serialize = "hive")]
    Hive,
    #[strum(serialize = "mssql")]
    MsSql,
    #[strum(serialize = "mysql")]
    MySql,
    #[strum(serialize = "postgres")]
    PostgreSql,
    #[strum(serialize = "sqlite")]
    SQLite,
    #[strum(serialize = "snowflake")]
    Snowflake,
}

// Is this the best approach for the Enum / Struct — basically that we have one
// Enum that gets its respective Struct, and then the Struct can also get its
// respective Enum?

impl Dialect {
    pub(super) fn handler(&self) -> Box<dyn DialectHandler> {
        match self {
            Dialect::MsSql => Box::new(MsSqlDialect),
            Dialect::MySql => Box::new(MySqlDialect),
            Dialect::BigQuery => Box::new(BigQueryDialect),
            Dialect::SQLite => Box::new(SQLiteDialect),
            Dialect::ClickHouse => Box::new(ClickHouseDialect),
            Dialect::Snowflake => Box::new(SnowflakeDialect),
            Dialect::DuckDb => Box::new(DuckDbDialect),
            Dialect::PostgreSql => Box::new(PostgresDialect),
            Dialect::Ansi | Dialect::Generic | Dialect::Hive => Box::new(GenericDialect),
        }
    }

    pub fn names() -> Vec<&'static str> {
        Dialect::iter()
            .flat_map(|d| d.get_serializations().to_vec())
            .collect::<Vec<&'static str>>()
    }
}

impl Default for Dialect {
    fn default() -> Self {
        Dialect::Generic
    }
}

pub struct GenericDialect;
pub struct SQLiteDialect;
pub struct MySqlDialect;
pub struct MsSqlDialect;
pub struct BigQueryDialect;
pub struct ClickHouseDialect;

pub struct SnowflakeDialect;
pub struct DuckDbDialect;
pub struct PostgresDialect;

pub(super) enum ColumnExclude {
    Exclude,
    Except,
}

pub(super) trait DialectHandler {
    fn use_top(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
    }

    fn big_query_quoting(&self) -> bool {
        false
    }

    fn column_exclude(&self) -> Option<ColumnExclude> {
        None
    }

    /// Support for DISTINCT in set ops (UNION DISTINCT, INTERSECT DISTINCT)
    /// When not supported we fallback to implicit DISTINCT.
    fn set_ops_distinct(&self) -> bool {
        true
    }

    /// Support or EXCEPT ALL.
    /// When not supported, fallback to anti join.
    fn except_all(&self) -> bool {
        true
    }

    fn intersect_all(&self) -> bool {
        self.except_all()
    }

    /// Support for CONCAT function.
    /// When not supported we fallback to use `||` as concat operator.
    fn has_concat_function(&self) -> bool {
        true
    }

    /// Whether or not intervals such as `INTERVAL 1 HOUR` require quotes like
    /// `INTERVAL '1' HOUR`
    fn requires_quotes_intervals(&self) -> bool {
        false
    }
}

impl DialectHandler for GenericDialect {}

impl DialectHandler for PostgresDialect {
    fn requires_quotes_intervals(&self) -> bool {
        true
    }
}

impl DialectHandler for SQLiteDialect {
    fn set_ops_distinct(&self) -> bool {
        false
    }

    fn except_all(&self) -> bool {
        false
    }

    fn has_concat_function(&self) -> bool {
        false
    }
}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }
}

impl DialectHandler for MySqlDialect {
    fn ident_quote(&self) -> char {
        '`'
    }

    fn set_ops_distinct(&self) -> bool {
        // https://dev.mysql.com/doc/refman/8.0/en/set-operations.html
        true
    }
}

impl DialectHandler for ClickHouseDialect {
    fn ident_quote(&self) -> char {
        '`'
    }
}

impl DialectHandler for BigQueryDialect {
    fn ident_quote(&self) -> char {
        '`'
    }
    fn big_query_quoting(&self) -> bool {
        true
    }
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#select_except
        Some(ColumnExclude::Except)
    }

    fn set_ops_distinct(&self) -> bool {
        // https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#set_operators
        true
    }
}

impl DialectHandler for SnowflakeDialect {
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://docs.snowflake.com/en/sql-reference/sql/select.html
        Some(ColumnExclude::Exclude)
    }

    fn set_ops_distinct(&self) -> bool {
        // https://docs.snowflake.com/en/sql-reference/operators-query.html
        false
    }
}

impl DialectHandler for DuckDbDialect {
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://duckdb.org/2022/05/04/friendlier-sql.html#select--exclude
        Some(ColumnExclude::Exclude)
    }

    fn except_all(&self) -> bool {
        // https://duckdb.org/docs/sql/query_syntax/setops.html
        false
    }
}

/*
## Set operations support matrix

Set-ops have quite different support in major SQL dialects. This is an attempt to document it.

| SQL construct                 | SQLite  | BQ     | Postgres | MySQL 8+ | DuckDB
|-------------------------------|---------|--------|----------|----------|--------
| UNION (implicit DISTINCT)     | x       |        | x        | x        | x
| UNION DISTINCT                |         | x      | x        | x        | x
| UNION ALL                     | x       | x      | x        | x        | x
| EXCEPT (implicit DISTINCT)    | x       |        | x        | x        | x
| EXCEPT DISTINCT               |         | x      | x        | x        | x
| EXCEPT ALL                    |         |        | x        | x        |


### UNION DISTINCT

For UNION, these are equivalent:
- a UNION DISTINCT b,
- DISTINCT (a UNION ALL b)
- DISTINCT (a UNION ALL (DISTINCT b))
- DISTINCT ((DISTINCT a) UNION ALL b)
- DISTINCT ((DISTINCT a) UNION ALL (DISTINCT b))


### EXCEPT DISTINCT

For EXCEPT it makes a difference when DISTINCT is applied. Below is a test query to validate
the behavior. When applied before EXCEPT, the output should be [3] and when applied after EXCEPT,
the output should be [2, 3].

```
SELECT * FROM (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 2 UNION ALL SELECT 3) t
EXCEPT
SELECT * FROM (SELECT 1 UNION ALL SELECT 2) t;
```

All dialects seem to be applying *before*, but none seem to document that.


### INTERSECT DISTINCT

For INTERSECT, it does not matter when DISTINCT is applied. BigQuery documentation does mention
it is applied *after*, which makes me think there is a difference I'm not seeing.

My reasoning is that:
- Distinct is equivalent to applying `group * (take 1)`.
- In effect, this is a restriction that "each group can have at most one value".
- If we apply DISTINCT to any input of INTERSECT ALL, this restriction on the input is retained
  through the operation. That's because no group will not contain more values than it started with,
  and no group that was present in both inputs, will be missing from the output.
- Thus, applying distinct after INTERSECT ALL is equivalent to applying it to any of the inputs.

*/
