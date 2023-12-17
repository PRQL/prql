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
use anyhow::{bail, Result};
use core::fmt::Debug;

use chrono::format::{Fixed, Item, Numeric, Pad, StrftimeItems};
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use strum::VariantNames;

/// SQL dialect.
///
/// This only changes the output for a relatively small subset of features.
///
/// If something does not work in a specific dialect, please raise in a
/// GitHub issue.
// Make sure to update Python bindings, JS bindings & docs in the book.
#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Serialize,
    Default,
    Deserialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumMessage,
    strum::EnumString,
    strum::EnumVariantNames,
)]
#[strum(serialize_all = "lowercase")]
pub enum Dialect {
    Ansi,
    BigQuery,
    ClickHouse,
    DuckDb,
    #[default]
    Generic,
    GlareDb,
    MsSql,
    MySql,
    Postgres,
    SQLite,
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
            Dialect::Postgres => Box::new(PostgresDialect),
            Dialect::GlareDb => Box::new(GlareDbDialect),
            Dialect::Ansi | Dialect::Generic => Box::new(GenericDialect),
        }
    }

    pub fn support_level(&self) -> SupportLevel {
        match self {
            Dialect::DuckDb
            | Dialect::SQLite
            | Dialect::Postgres
            | Dialect::MySql
            | Dialect::Generic
            | Dialect::GlareDb
            | Dialect::ClickHouse => SupportLevel::Supported,
            Dialect::MsSql | Dialect::Ansi | Dialect::BigQuery | Dialect::Snowflake => {
                SupportLevel::Unsupported
            }
        }
    }

    #[deprecated(note = "Use `Dialect::VARIANTS` instead")]
    pub fn names() -> &'static [&'static str] {
        Dialect::VARIANTS
    }
}

pub enum SupportLevel {
    Supported,
    Unsupported,
    Nascent,
}

#[derive(Debug)]
pub struct GenericDialect;
#[derive(Debug)]
pub struct SQLiteDialect;
#[derive(Debug)]
pub struct MySqlDialect;
#[derive(Debug)]
pub struct MsSqlDialect;
#[derive(Debug)]
pub struct BigQueryDialect;
#[derive(Debug)]
pub struct ClickHouseDialect;
#[derive(Debug)]
pub struct SnowflakeDialect;
#[derive(Debug)]
pub struct DuckDbDialect;
#[derive(Debug)]
pub struct PostgresDialect;
#[derive(Debug)]
pub struct GlareDbDialect;

pub(super) enum ColumnExclude {
    Exclude,
    Except,
}

/// All the chrono items supported by PRQL
fn is_supported_chrono_item(item: &Item) -> bool {
    matches!(
        item,
        Item::Numeric(Numeric::Year, Pad::Zero)
            | Item::Numeric(Numeric::YearMod100, Pad::Zero)
            | Item::Numeric(Numeric::Month, Pad::None)
            | Item::Numeric(Numeric::Month, Pad::Zero)
            | Item::Numeric(Numeric::Day, Pad::None)
            | Item::Numeric(Numeric::Day, Pad::Zero)
            | Item::Numeric(Numeric::Hour, Pad::None)
            | Item::Numeric(Numeric::Hour, Pad::Zero)
            | Item::Numeric(Numeric::Hour12, Pad::Zero)
            | Item::Numeric(Numeric::Minute, Pad::Zero)
            | Item::Numeric(Numeric::Second, Pad::Zero)
            | Item::Numeric(Numeric::Nanosecond, Pad::Zero)
            | Item::Fixed(Fixed::ShortMonthName)
            | Item::Fixed(Fixed::LongMonthName)
            | Item::Fixed(Fixed::ShortWeekdayName)
            | Item::Fixed(Fixed::LongWeekdayName)
            | Item::Fixed(Fixed::UpperAmPm)
            | Item::Fixed(Fixed::RFC3339)
            | Item::Literal(_)
            | Item::Space(_)
    )
}

pub(super) trait DialectHandler: Any + Debug {
    fn use_top(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
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

    /// Support for GROUP BY *
    fn stars_in_group(&self) -> bool {
        true
    }

    fn supports_distinct_on(&self) -> bool {
        false
    }

    /// Get the date format for the given dialect
    /// PRQL uses the same format as `chrono` crate
    /// (see https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
    fn translate_prql_date_format(&self, prql_date_format: &str) -> Result<String> {
        Ok(StrftimeItems::new(prql_date_format)
            .map(|item| {
                if !is_supported_chrono_item(&item) {
                    bail!("PRQL doesn't support this date format item")
                }
                self.translate_chrono_item(&item)
            })
            .collect::<Result<Vec<_>>>()?
            .join(""))
    }

    fn translate_chrono_item(&self, _item: &Item) -> Result<String> {
        bail!("Date formatting is not yet supported for this dialect")
    }
}

impl dyn DialectHandler {
    #[inline]
    pub fn is<T: DialectHandler + 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}

impl DialectHandler for GenericDialect {
    fn translate_chrono_item(&self, _item: &Item) -> Result<String> {
        bail!("Date formatting requires a dialect")
    }
}

impl DialectHandler for PostgresDialect {
    fn requires_quotes_intervals(&self) -> bool {
        true
    }

    fn supports_distinct_on(&self) -> bool {
        true
    }
}

impl DialectHandler for GlareDbDialect {
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

    fn stars_in_group(&self) -> bool {
        false
    }
}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }

    // https://learn.microsoft.com/en-us/sql/t-sql/language-elements/set-operators-except-and-intersect-transact-sql?view=sql-server-ver16
    fn except_all(&self) -> bool {
        false
    }

    fn set_ops_distinct(&self) -> bool {
        false
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

    fn supports_distinct_on(&self) -> bool {
        true
    }
}

impl DialectHandler for BigQueryDialect {
    fn ident_quote(&self) -> char {
        '`'
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

    fn supports_distinct_on(&self) -> bool {
        true
    }

    // https://duckdb.org/docs/sql/functions/dateformat
    fn translate_chrono_item<'a>(&self, item: &Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "%Y".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "%y".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "%-m".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "%m".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "%-d".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "%d".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "%-H".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "%H".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "%I".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "%M".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "%S".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "%f".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "%b".to_string(),
            Item::Fixed(Fixed::LongMonthName) => "%B".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "%a".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "%A".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "%p".to_string(),
            Item::Fixed(Fixed::RFC3339) => "%Y-%m-%dT%H:%M:%S.%fZ".to_string(),
            Item::Literal(literal) => literal.replace('\'', "''").replace('%', "%%"),
            Item::Space(spaces) => spaces.to_string(),
            _ => unreachable!("only supported chrono items by PRQL are translated"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Dialect;
    use insta::assert_debug_snapshot;
    use std::str::FromStr;

    #[test]
    fn test_dialect_from_str() {
        assert_debug_snapshot!(Dialect::from_str("postgres"), @r###"
        Ok(
            Postgres,
        )
        "###);

        assert_debug_snapshot!(Dialect::from_str("foo"), @r###"
        Err(
            VariantNotFound,
        )
        "###);
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
