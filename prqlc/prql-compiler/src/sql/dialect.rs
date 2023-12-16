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
    fn translate_prql_format(&self, prql_format: &str) -> String {
        StrftimeItems::new(prql_format)
            .map(|item| match item {
                Item::Numeric(numeric, pad) => {
                    self.convert_date_numeric_item(numeric, pad).to_string()
                }
                Item::Fixed(fixed) => self.convert_date_fixed_item(fixed).to_string(),
                Item::Literal(literal) => {
                    if literal
                        .chars()
                        .any(|c| c.is_ascii_alphanumeric() || c == '"' || c == '\'')
                    {
                        self.escape_alpha_numeric_literal(literal)
                    } else {
                        literal.to_string()
                    }
                }
                Item::OwnedLiteral(literal) => literal.to_string(),
                Item::Space(spaces) => spaces.to_string(),
                Item::OwnedSpace(spaces) => spaces.to_string(),
                Item::Error => panic!("invalid format"),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str;

    fn convert_date_fixed_item(&self, item: Fixed) -> &str;

    fn escape_alpha_numeric_literal(&self, literal: &str) -> String {
        literal.to_string()
    }
}

impl dyn DialectHandler {
    #[inline]
    pub fn is<T: DialectHandler + 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}

impl DialectHandler for GenericDialect {
    fn convert_date_fixed_item(&self, _item: Fixed) -> &str {
        unimplemented!("GenericDialect does not support date formatting")
    }

    fn convert_date_numeric_item(&self, _item: Numeric, _pad: Pad) -> &str {
        unimplemented!("GenericDialect does not support date formatting")
    }
}

impl DialectHandler for PostgresDialect {
    fn requires_quotes_intervals(&self) -> bool {
        true
    }

    fn supports_distinct_on(&self) -> bool {
        true
    }

    // https://www.postgresql.org/docs/current/functions-formatting.html#FUNCTIONS-FORMATTING-DATETIME-TABLE
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "YYYY",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => match pad {
                Pad::Zero => "YY",
                Pad::None | Pad::Space => "FMYY",
            },
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "MM",
                Pad::None | Pad::Space => "FMMM",
            },
            Numeric::Day => match pad {
                Pad::Zero => "DD",
                Pad::None | Pad::Space => "FMDD",
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => "IW",
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => "DDD",
            Numeric::Hour => match pad {
                Pad::Zero => "HH24",
                Pad::None | Pad::Space => "FMHH24",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "HH12",
                Pad::None | Pad::Space => "FMHH12",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "MI",
                Pad::None | Pad::Space => "FMMI",
            },
            Numeric::Second => match pad {
                Pad::Zero => "SS",
                Pad::None | Pad::Space => "FMSS",
            },
            Numeric::Nanosecond => unimplemented!(),
            Numeric::Timestamp => unimplemented!(),
            Numeric::Internal(_) => unreachable!("internal type"),
        }
    }

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            // By default long names are blank-padded to 9 chars
            // We trim them with the FM prefix.
            Fixed::ShortMonthName => "Mon",
            Fixed::LongMonthName => "FMMonth",
            Fixed::ShortWeekdayName => "Dy",
            Fixed::LongWeekdayName => "FMDay",
            Fixed::LowerAmPm => "am",
            Fixed::UpperAmPm => "AM",
            Fixed::Nanosecond => unimplemented!(),
            Fixed::Nanosecond3 => "MS",
            Fixed::Nanosecond6 => "US",
            Fixed::Nanosecond9 => unimplemented!(),
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }

    fn escape_alpha_numeric_literal(&self, literal: &str) -> String {
        format!("\"{}\"", literal.replace('\'', "''").replace('\"', "\\\""))
    }
}

impl DialectHandler for GlareDbDialect {
    fn requires_quotes_intervals(&self) -> bool {
        true
    }

    fn convert_date_fixed_item(&self, _item: Fixed) -> &str {
        unimplemented!("GlareDB does not support date formatting")
    }

    fn convert_date_numeric_item(&self, _item: Numeric, _pad: Pad) -> &str {
        unimplemented!("GlareDB does not support date formatting")
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

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            // SQLite does not support day names
            // https://www.sqlite.org/lang_datefunc.html
            // TODO: case on '%w': 0 => 'Sun', 1 => 'Mon', ...
            Fixed::ShortMonthName => unimplemented!(),
            Fixed::LongMonthName => unimplemented!(),
            // TODO: case on '%m': 01 => 'Jan', 02 => 'Feb', ...
            Fixed::ShortWeekdayName => unimplemented!(),
            Fixed::LongWeekdayName => unimplemented!(),
            Fixed::LowerAmPm => "%P",
            Fixed::UpperAmPm => "%p",
            Fixed::Nanosecond => unimplemented!(),
            Fixed::Nanosecond3 => unimplemented!(),
            Fixed::Nanosecond6 => "%f",
            Fixed::Nanosecond9 => unimplemented!(),
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "%Y-%m-%dT%H:%M:%S.%fZ",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }

    // https://www.sqlite.org/lang_datefunc.html
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "%Y",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => unimplemented!(),
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "%m",
                Pad::None | Pad::Space => unimplemented!(),
            },
            Numeric::Day => match pad {
                Pad::Zero => "%d",
                Pad::None | Pad::Space => unimplemented!(),
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => unimplemented!(),
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => unimplemented!(),
            Numeric::Hour => match pad {
                Pad::Zero => "%H",
                Pad::None | Pad::Space => "%k",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "%I",
                Pad::None | Pad::Space => "%l",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "%M",
                Pad::None | Pad::Space => unimplemented!("No padding is not supported for minutes"),
            },
            Numeric::Second => match pad {
                Pad::Zero => "%S",
                Pad::None | Pad::Space => unimplemented!("No padding is not supported for seconds"),
            },
            Numeric::Nanosecond => unimplemented!(),
            Numeric::Timestamp => "%s",
            _ => panic!("invalid format"),
        }
    }
}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }

    // https://learn.microsoft.com/en-us/sql/t-sql/language-elements/set-operators-except-and-intersect-transact-sql
    fn except_all(&self) -> bool {
        false
    }

    fn set_ops_distinct(&self) -> bool {
        false
    }

    // https://learn.microsoft.com/en-us/dotnet/standard/base-types/custom-date-and-time-format-strings
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "yyyy",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => match pad {
                Pad::Zero => "yy",
                Pad::None | Pad::Space => "y",
            },
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "MM",
                Pad::None | Pad::Space => "M",
            },
            Numeric::Day => match pad {
                Pad::Zero => "dd",
                Pad::None | Pad::Space => "d",
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => unimplemented!(),
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => unimplemented!(),
            Numeric::Hour => match pad {
                Pad::Zero => "HH",
                Pad::None | Pad::Space => "H",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "hh",
                Pad::None | Pad::Space => "h",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "mm",
                Pad::None | Pad::Space => "m",
            },
            Numeric::Second => match pad {
                Pad::Zero => "ss",
                Pad::None | Pad::Space => "s",
            },
            Numeric::Nanosecond => unimplemented!(),
            Numeric::Timestamp => unimplemented!(),
            Numeric::Internal(_) => unreachable!("internal type"),
        }
    }

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            Fixed::ShortMonthName => "MMM",
            Fixed::LongMonthName => "MMMM",
            Fixed::ShortWeekdayName => "ddd",
            Fixed::LongWeekdayName => "dddd",
            Fixed::LowerAmPm => unimplemented!(),
            Fixed::UpperAmPm => "tt",
            Fixed::Nanosecond => unimplemented!(),
            Fixed::Nanosecond3 => "fff",
            Fixed::Nanosecond6 => "ffffff",
            Fixed::Nanosecond9 => unimplemented!(),
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "yyyy-MM-dd'T'HH:mm:ss.ffffff'Z'",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }

    fn escape_alpha_numeric_literal(&self, literal: &str) -> String {
        format!("\"{}\"", literal.replace('\'', "''").replace('\"', "\\\""))
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

    // https://dev.mysql.com/doc/refman/8.0/en/date-and-time-functions.html#function_date-format
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "%Y",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => match pad {
                Pad::Zero => "%y",
                Pad::None | Pad::Space => unimplemented!("No padding is not supported for year"),
            },
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "%m",
                Pad::None | Pad::Space => "%c",
            },
            Numeric::Day => match pad {
                Pad::Zero => "%d",
                Pad::None | Pad::Space => "%e",
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => unimplemented!(),
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => unimplemented!(),
            Numeric::Hour => match pad {
                Pad::Zero => "%H",
                Pad::None | Pad::Space => "%k",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "%I",
                Pad::None | Pad::Space => "%l",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "%i",
                Pad::None | Pad::Space => unimplemented!("No padding is not supported for minutes"),
            },
            Numeric::Second => match pad {
                Pad::Zero => "%S",
                Pad::None | Pad::Space => unimplemented!("No padding is not supported for seconds"),
            },
            Numeric::Nanosecond => unimplemented!(),
            Numeric::Timestamp => unimplemented!(),
            _ => panic!("invalid format"),
        }
    }

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            Fixed::ShortMonthName => "%b",
            Fixed::LongMonthName => "%M",
            Fixed::ShortWeekdayName => "%a",
            Fixed::LongWeekdayName => "%W",
            Fixed::LowerAmPm => unimplemented!(),
            Fixed::UpperAmPm => "%p",
            Fixed::Nanosecond => unimplemented!(),
            Fixed::Nanosecond3 => unimplemented!(),
            Fixed::Nanosecond6 => "%f",
            Fixed::Nanosecond9 => unimplemented!(),
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "%Y-%m-%dT%H:%i:%S.%fZ",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }
}

impl DialectHandler for ClickHouseDialect {
    fn ident_quote(&self) -> char {
        '`'
    }

    fn supports_distinct_on(&self) -> bool {
        true
    }

    // https://clickhouse.com/docs/en/sql-reference/functions/date-time-functions#formatDateTime
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "yyyy",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => match pad {
                Pad::Zero => "yy",
                Pad::None | Pad::Space => "y",
            },
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "MM",
                Pad::None | Pad::Space => "M",
            },
            Numeric::Day => match pad {
                Pad::Zero => "dd",
                Pad::None | Pad::Space => "d",
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => unimplemented!(),
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => unimplemented!(),
            Numeric::Hour => match pad {
                Pad::Zero => "HH",
                Pad::None | Pad::Space => "H",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "hh",
                Pad::None | Pad::Space => "h",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "mm",
                Pad::None | Pad::Space => "m",
            },
            Numeric::Second => match pad {
                Pad::Zero => "ss",
                Pad::None | Pad::Space => "s",
            },
            Numeric::Nanosecond => unimplemented!(),
            Numeric::Timestamp => unimplemented!(),
            _ => panic!("invalid format"),
        }
    }

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            Fixed::ShortMonthName => "MMM",
            Fixed::LongMonthName => "MMMM",
            Fixed::ShortWeekdayName => "EEE",
            Fixed::LongWeekdayName => "EEEE",
            Fixed::LowerAmPm => unimplemented!(),
            Fixed::UpperAmPm => "aa",
            Fixed::Nanosecond => unimplemented!(),
            Fixed::Nanosecond3 => unimplemented!(),
            Fixed::Nanosecond6 => "SSSSSS",
            Fixed::Nanosecond9 => unimplemented!(),
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "yyyy-MM-dd'T'HH:mm:ss.SSSSSS'Z'",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }

    fn escape_alpha_numeric_literal(&self, literal: &str) -> String {
        format!("'{}'", literal.replace('\'', "''").replace('\"', "\\\""))
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

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        MySqlDialect.convert_date_fixed_item(item)
    }

    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        MySqlDialect.convert_date_numeric_item(item, pad)
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

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        PostgresDialect.convert_date_fixed_item(item)
    }

    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        PostgresDialect.convert_date_numeric_item(item, pad)
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
    fn convert_date_numeric_item(&self, item: Numeric, pad: Pad) -> &str {
        match item {
            Numeric::Year => "%Y",
            Numeric::YearDiv100 => unimplemented!(),
            Numeric::YearMod100 => match pad {
                Pad::Zero => "%y",
                // We can't add a space only for values greater than 9, so we use the same
                Pad::None | Pad::Space => "%-y",
            },
            Numeric::IsoYear => unimplemented!(),
            Numeric::IsoYearDiv100 => unimplemented!(),
            Numeric::IsoYearMod100 => unimplemented!(),
            Numeric::Month => match pad {
                Pad::Zero => "%m",
                Pad::None | Pad::Space => "%-m",
            },
            Numeric::Day => match pad {
                Pad::Zero => "%d",
                Pad::None | Pad::Space => "%-d",
            },
            Numeric::WeekFromSun => unimplemented!(),
            Numeric::WeekFromMon => unimplemented!(),
            Numeric::IsoWeek => unimplemented!(),
            Numeric::NumDaysFromSun => unimplemented!(),
            Numeric::WeekdayFromMon => unimplemented!(),
            Numeric::Ordinal => unimplemented!(),
            Numeric::Hour => match pad {
                Pad::Zero => "%H",
                Pad::None | Pad::Space => "%-H",
            },
            Numeric::Hour12 => match pad {
                Pad::Zero => "%I",
                Pad::None | Pad::Space => "%-I",
            },
            Numeric::Minute => match pad {
                Pad::Zero => "%M",
                Pad::None | Pad::Space => "%-M",
            },
            Numeric::Second => match pad {
                Pad::Zero => "%S",
                Pad::None | Pad::Space => "%-S",
            },
            Numeric::Nanosecond => "%n",
            Numeric::Timestamp => unimplemented!(),
            _ => panic!("invalid format"),
        }
    }

    fn convert_date_fixed_item(&self, item: Fixed) -> &str {
        match item {
            Fixed::ShortMonthName => "%b",
            Fixed::LongMonthName => "%B",
            Fixed::ShortWeekdayName => "%a",
            Fixed::LongWeekdayName => "%A",
            Fixed::LowerAmPm => unimplemented!(),
            Fixed::UpperAmPm => "%p",
            Fixed::Nanosecond => "%n",
            Fixed::Nanosecond3 => "%g",
            Fixed::Nanosecond6 => "%f",
            Fixed::Nanosecond9 => "%n",
            Fixed::TimezoneName => unimplemented!(),
            Fixed::TimezoneOffsetColon => unimplemented!(),
            Fixed::TimezoneOffsetDoubleColon => unimplemented!(),
            Fixed::TimezoneOffsetTripleColon => unimplemented!(),
            Fixed::TimezoneOffsetColonZ => unimplemented!(),
            Fixed::TimezoneOffset => unimplemented!(),
            Fixed::TimezoneOffsetZ => unimplemented!(),
            Fixed::RFC3339 => "%Y-%m-%dT%H:%M:%S.%fZ",
            Fixed::RFC2822 | Fixed::Internal(_) => unreachable!("cannot be used in format"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
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

    #[rstest]
    #[case("%Y-%m-%d", DuckDbDialect {},"%Y-%m-%d")]
    #[case("%Y-%m-%d", MsSqlDialect {}, "yyyy-MM-dd")]
    #[case("%Y-%m-%d", MySqlDialect {},"%Y-%m-%d")]
    #[case("%Y-%m-%d", PostgresDialect {}, "YYYY-MM-DD")]
    #[case("%Y-%m-%d %H:%M:%S.%.6f", DuckDbDialect {}, "%Y-%m-%d %H:%M:%S.%f")]
    #[case("%Y-%m-%d %H:%M:%S.%.6f", MsSqlDialect {}, "yyyy-MM-dd HH:mm:ss.ffffff")]
    #[case("%Y-%m-%d %H:%M:%S.%.6f", MySqlDialect {}, "%Y-%m-%d %H:%i:%S.%f")]
    #[case("%Y-%m-%d %H:%M:%S.%.6f", PostgresDialect {}, "YYYY-MM-DD HH24:MI:SS.US")]
    #[case("%A %B %d %Y", DuckDbDialect {},"%A %B %d %Y")]
    #[case("%A %B %d %Y", MsSqlDialect {}, "dddd MMMM dd yyyy")]
    #[case("%A %B %d %Y", MySqlDialect {},"%W %M %d %Y")]
    #[case("%A %B %d %Y", PostgresDialect {}, "FMDay FMMonth DD YYYY")]
    fn test_translate_date(
        #[case] prql_date_format: &str,
        #[case] dialect: impl DialectHandler + 'static,
        #[case] expected_dialect_date_format: &str,
    ) {
        assert_eq!(
            dialect.translate_prql_format(prql_date_format),
            expected_dialect_date_format.to_string()
        );
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
