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
use std::any::{Any, TypeId};

use chrono::format::{Fixed, Item, Numeric, Pad, StrftimeItems};
use serde::{Deserialize, Serialize};
use sqlparser::ast::DateTimeField;
use strum::VariantNames;

use crate::{Error, Result};

/// Convert a chrono format `Item` back to its strftime string representation.
fn chrono_item_to_strftime(item: &Item) -> String {
    let pad_char = |pad: &Pad| match pad {
        Pad::None => "-",
        Pad::Zero => "",
        Pad::Space => "_",
    };
    let numeric_char = |num: &Numeric| -> String {
        match num {
            Numeric::Year => "Y",
            Numeric::YearMod100 => "y",
            Numeric::Month => "m",
            Numeric::Day => "d",
            Numeric::Hour => "H",
            Numeric::Hour12 => "I",
            Numeric::Minute => "M",
            Numeric::Second => "S",
            Numeric::Nanosecond => "f",
            _ => return format!("{num:?}"),
        }
        .to_string()
    };
    match item {
        Item::Numeric(num, pad) => format!("%{}{}", pad_char(pad), numeric_char(num)),
        Item::Fixed(Fixed::ShortMonthName) => "%b".to_string(),
        Item::Fixed(Fixed::LongMonthName) => "%B".to_string(),
        Item::Fixed(Fixed::ShortWeekdayName) => "%a".to_string(),
        Item::Fixed(Fixed::LongWeekdayName) => "%A".to_string(),
        Item::Fixed(Fixed::UpperAmPm) => "%p".to_string(),
        Item::Fixed(Fixed::LowerAmPm) => "%P".to_string(),
        Item::Fixed(Fixed::RFC3339) => "%+".to_string(),
        _ => format!("{item:?}"),
    }
}

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
    strum::VariantNames,
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
    Redshift,
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
            Dialect::Redshift => Box::new(RedshiftDialect),
            Dialect::GlareDb => Box::new(GlareDbDialect),
            Dialect::Ansi | Dialect::Generic => Box::new(GenericDialect),
        }
    }

    pub fn support_level(&self) -> SupportLevel {
        match self {
            Dialect::DuckDb
            | Dialect::SQLite
            | Dialect::Postgres
            | Dialect::Redshift
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
pub struct RedshiftDialect;
#[derive(Debug)]
pub struct GlareDbDialect;

pub(super) enum ColumnExclude {
    Exclude,
    Except,
}

pub enum IdentQuotingStyle {
    AlwaysQuoted,
    ConditionallyQuoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalQuotingStyle {
    // INTERVAL 1 day
    NoQuotes,
    // INTERVAL '1' day
    ValueQuoted,
    // INTERVAL '1 day'
    ValueAndUnitQuoted,
}

pub(super) trait DialectHandler: Any + Debug {
    fn use_fetch(&self) -> bool {
        false
    }

    fn ident_quote(&self) -> char {
        '"'
    }

    fn ident_quoting_style(&self) -> IdentQuotingStyle {
        IdentQuotingStyle::ConditionallyQuoted
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
    /// `INTERVAL '1 HOUR'` or `INTERVAL '1' HOUR`
    fn interval_quoting_style(&self, _dtf: &DateTimeField) -> IntervalQuotingStyle {
        IntervalQuotingStyle::NoQuotes
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
            .map(|item| self.translate_chrono_item(item))
            .collect::<Result<Vec<_>>>()?
            .join(""))
    }

    fn translate_chrono_item(&self, _item: Item) -> Result<String> {
        Err(Error::new_simple(
            "Date formatting is not yet supported for this dialect",
        ))
    }

    fn supports_zero_columns(&self) -> bool {
        false
    }

    fn translate_sql_array(
        &self,
        elements: Vec<sqlparser::ast::Expr>,
    ) -> crate::Result<sqlparser::ast::Expr> {
        use sqlparser::ast::Expr;

        // Default SQL syntax: [elem1, elem2, ...]
        Ok(Expr::Array(sqlparser::ast::Array {
            elem: elements,
            named: false,
        }))
    }

    /// Whether source and subqueries should be put between simple parentheses
    /// for `UNION` and similar verbs.
    fn prefers_subquery_parentheses_shorthand(&self) -> bool {
        false
    }

    /// Whether window functions require an ORDER BY clause.
    /// Snowflake requires ORDER BY for ranking functions like ROW_NUMBER().
    fn requires_order_by_in_window_function(&self) -> bool {
        false
    }
}

impl dyn DialectHandler {
    #[inline]
    pub fn is<T: DialectHandler + 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}

impl DialectHandler for GenericDialect {
    fn translate_chrono_item(&self, _item: Item) -> Result<String> {
        Err(Error::new_simple("Date formatting requires a dialect"))
    }
}

impl DialectHandler for PostgresDialect {
    fn interval_quoting_style(&self, _dtf: &DateTimeField) -> IntervalQuotingStyle {
        IntervalQuotingStyle::ValueAndUnitQuoted
    }

    fn supports_distinct_on(&self) -> bool {
        true
    }

    // https://www.postgresql.org/docs/current/functions-formatting.html
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "YYYY".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "YY".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "FMMM".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "MM".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "FMDD".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "DD".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "FMHH24".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "HH24".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "HH12".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "MI".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "SS".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "US".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "Mon".to_string(),
            // By default long names are blank-padded to 9 chars so we need to use FM prefix
            Item::Fixed(Fixed::LongMonthName) => "FMMonth".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "Dy".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "FMDay".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "AM".to_string(),
            Item::Fixed(Fixed::RFC3339) => "YYYY-MM-DD\"T\"HH24:MI:SS.USZ".to_string(),
            Item::Literal(literal) => {
                // literals are split at every non alphanumeric character
                if literal.chars().any(|c| c.is_ascii_alphanumeric()) {
                    // If the literal contains alphanumeric characters, we need to quote it
                    // to avoid it being interpreted as a pattern understood by Postgres.
                    // We hence need to put it in double quotes to force it to be interpreted as literal text
                    format!("\"{literal}\"")
                } else {
                    literal.replace('\'', "''").replace('"', "\\\"")
                }
            }
            Item::Space(spaces) => spaces.to_string(),
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
    }

    fn supports_zero_columns(&self) -> bool {
        true
    }

    fn prefers_subquery_parentheses_shorthand(&self) -> bool {
        true
    }
}

impl DialectHandler for RedshiftDialect {
    fn ident_quoting_style(&self) -> IdentQuotingStyle {
        // Use conditional quoting with dialect-specific keywords
        IdentQuotingStyle::ConditionallyQuoted
    }

    fn interval_quoting_style(&self, dtf: &DateTimeField) -> IntervalQuotingStyle {
        if matches!(dtf, DateTimeField::Week(_) | DateTimeField::Weeks) {
            IntervalQuotingStyle::ValueAndUnitQuoted
        } else {
            IntervalQuotingStyle::ValueQuoted
        }
    }

    fn supports_distinct_on(&self) -> bool {
        false
    }

    // https://docs.aws.amazon.com/redshift/latest/dg/r_FORMAT_strings.html
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "YYYY".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "YY".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "FMMM".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "MM".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "FMDD".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "DD".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "FMHH24".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "HH24".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "HH12".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "MI".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "SS".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "US".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "Mon".to_string(),
            // By default long names are blank-padded to 9 chars so we need to use FM prefix
            Item::Fixed(Fixed::LongMonthName) => "FMMonth".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "Dy".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "FMDay".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "AM".to_string(),
            Item::Fixed(Fixed::RFC3339) => "YYYY-MM-DD\"T\"HH24:MI:SS.USZ".to_string(),
            Item::Literal(literal) => {
                // literals are split at every non alphanumeric character
                if literal.chars().any(|c| c.is_ascii_alphanumeric()) {
                    // If the literal contains alphanumeric characters, we need to quote it
                    // to avoid it being interpreted as a pattern understood by Redshift.
                    // We hence need to put it in double quotes to force it to be interpreted as literal text
                    format!("\"{literal}\"")
                } else {
                    literal.replace('\'', "''").replace('"', "\\\"")
                }
            }
            Item::Space(spaces) => spaces.to_string(),
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
    }

    fn supports_zero_columns(&self) -> bool {
        true
    }

    fn prefers_subquery_parentheses_shorthand(&self) -> bool {
        true
    }

    // Redshift only supports CONCAT with two elements, the docs recommend to use the || operator
    // for more than two elements: https://docs.aws.amazon.com/redshift/latest/dg/r_CONCAT.html
    fn has_concat_function(&self) -> bool {
        false
    }
}

impl DialectHandler for GlareDbDialect {
    fn interval_quoting_style(&self, _dtf: &DateTimeField) -> IntervalQuotingStyle {
        IntervalQuotingStyle::ValueAndUnitQuoted
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
    fn use_fetch(&self) -> bool {
        true
    }

    // https://learn.microsoft.com/en-us/sql/t-sql/language-elements/set-operators-except-and-intersect-transact-sql?view=sql-server-ver16
    fn except_all(&self) -> bool {
        false
    }

    fn set_ops_distinct(&self) -> bool {
        false
    }

    // https://learn.microsoft.com/en-us/dotnet/standard/base-types/custom-date-and-time-format-strings
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "yyyy".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "yy".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "M".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "MM".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "d".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "dd".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "H".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "HH".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "hh".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "mm".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "ss".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "ffffff".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "MMM".to_string(),
            Item::Fixed(Fixed::LongMonthName) => "MMMM".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "ddd".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "dddd".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "tt".to_string(),
            Item::Fixed(Fixed::RFC3339) => "yyyy-MM-dd'T'HH:mm:ss.ffffff'Z'".to_string(),
            Item::Literal(literal) => {
                // literals are split at every non alphanumeric character
                if literal.chars().any(|c| c.is_ascii_alphanumeric()) {
                    // If the literal contains alphanumeric characters, we need to quote it
                    // to avoid it being interpreted as a pattern understood by MSSQL.
                    // We hence need to put it in double quotes to force it to be interpreted as literal text
                    format!("\"{literal}\"")
                } else {
                    // MSSQL uses single quotes around
                    literal
                        .replace('"', "\\\"")
                        .replace('\'', "\"\'\"")
                        .replace('%', "\\%")
                }
            }
            Item::Space(spaces) => spaces.to_string(),
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
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
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "%Y".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "%y".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "%c".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "%m".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "%e".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "%d".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "%k".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "%H".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "%I".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "%i".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "%S".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "%f".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "%b".to_string(),
            Item::Fixed(Fixed::LongMonthName) => "%M".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "%a".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "%W".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "%p".to_string(),
            Item::Fixed(Fixed::RFC3339) => "%Y-%m-%dT%H:%i:%S.%fZ".to_string(),
            Item::Literal(literal) => literal.replace('\'', "''").replace('%', "%%"),
            Item::Space(spaces) => spaces.to_string(),
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
    }
}

impl DialectHandler for ClickHouseDialect {
    fn ident_quote(&self) -> char {
        '`'
    }

    fn supports_distinct_on(&self) -> bool {
        true
    }

    // https://clickhouse.com/docs/en/sql-reference/functions/date-time-functions#formatDateTimeInJodaSyntax
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "yyyy".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "yy".to_string(),
            Item::Numeric(Numeric::Month, Pad::None) => "M".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "MM".to_string(),
            Item::Numeric(Numeric::Day, Pad::None) => "d".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "dd".to_string(),
            Item::Numeric(Numeric::Hour, Pad::None) => "H".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "HH".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "hh".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "mm".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "ss".to_string(),
            Item::Numeric(Numeric::Nanosecond, Pad::Zero) => "SSSSSS".to_string(), // Microseconds
            Item::Fixed(Fixed::ShortMonthName) => "MMM".to_string(),
            Item::Fixed(Fixed::LongMonthName) => "MMMM".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "EEE".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "EEEE".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "aa".to_string(),
            Item::Fixed(Fixed::RFC3339) => "yyyy-MM-dd'T'HH:mm:ss.SSSSSS'Z'".to_string(),
            Item::Literal(literal) => {
                // literals are split at every non alphanumeric character
                if literal.chars().any(|c| c.is_ascii_alphanumeric()) {
                    // If the literal contains alphanumeric characters, we need to quote it
                    // to avoid it being interpreted as a pattern understood by Clickhouse.
                    // Clickhouse uses backticks around
                    format!("'{literal}'")
                } else {
                    literal.replace('\'', "\\'\\'")
                }
            }
            Item::Space(spaces) => spaces.to_string(),
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
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

    fn prefers_subquery_parentheses_shorthand(&self) -> bool {
        true
    }

    // https://cloud.google.com/bigquery/docs/reference/standard-sql/format-elements#format_elements_date_time
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
        Ok(match item {
            Item::Numeric(Numeric::Year, Pad::Zero) => "%Y".to_string(),
            Item::Numeric(Numeric::YearMod100, Pad::Zero) => "%y".to_string(),
            Item::Numeric(Numeric::Month, Pad::Zero) => "%m".to_string(),
            Item::Numeric(Numeric::Day, Pad::Zero) => "%d".to_string(),
            Item::Numeric(Numeric::Hour, Pad::Zero) => "%H".to_string(),
            Item::Numeric(Numeric::Hour12, Pad::Zero) => "%I".to_string(),
            Item::Numeric(Numeric::Minute, Pad::Zero) => "%M".to_string(),
            Item::Numeric(Numeric::Second, Pad::Zero) => "%S".to_string(),
            Item::Fixed(Fixed::ShortMonthName) => "%b".to_string(),
            Item::Fixed(Fixed::LongMonthName) => "%B".to_string(),
            Item::Fixed(Fixed::ShortWeekdayName) => "%a".to_string(),
            Item::Fixed(Fixed::LongWeekdayName) => "%A".to_string(),
            Item::Fixed(Fixed::UpperAmPm) => "%p".to_string(),
            Item::Fixed(Fixed::RFC3339) => "%Y-%m-%dT%H:%M:%S%Ez".to_string(),
            Item::Literal(literal) => literal.replace('\'', "''").replace('%', "%%"),
            Item::Space(spaces) => spaces.to_string(),
            item => {
                return Err(Error::new_simple(format!(
                    "format specifier `{}` is not supported for BigQuery",
                    chrono_item_to_strftime(&item),
                )))
            }
        })
    }
}

impl DialectHandler for SnowflakeDialect {
    fn ident_quoting_style(&self) -> IdentQuotingStyle {
        // Due to snowflake's identifier casing rules, identifiers are always quoted
        // https://docs.snowflake.com/en/sql-reference/identifiers-syntax#label-identifier-casing
        IdentQuotingStyle::AlwaysQuoted
    }

    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://docs.snowflake.com/en/sql-reference/sql/select.html
        Some(ColumnExclude::Exclude)
    }

    fn set_ops_distinct(&self) -> bool {
        // https://docs.snowflake.com/en/sql-reference/operators-query.html
        false
    }

    fn interval_quoting_style(&self, _dtf: &DateTimeField) -> IntervalQuotingStyle {
        // Snowflake requires quotes around value and unit together
        // https://docs.snowflake.com/en/sql-reference/data-types-datetime#interval-constants
        IntervalQuotingStyle::ValueAndUnitQuoted
    }

    fn requires_order_by_in_window_function(&self) -> bool {
        // https://docs.snowflake.com/en/sql-reference/functions/row_number
        // ROW_NUMBER() requires ORDER BY in window specification
        true
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
    fn translate_chrono_item<'a>(&self, item: Item) -> Result<String> {
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
            _ => {
                return Err(Error::new_simple(
                    "PRQL doesn't support this format specifier",
                ))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::format::{Fixed, Item, Numeric, Pad};
    use insta::{assert_debug_snapshot, assert_snapshot};

    use super::{chrono_item_to_strftime, BigQueryDialect, Dialect, DialectHandler};

    #[test]
    fn test_dialect_from_str() {
        assert_debug_snapshot!(Dialect::from_str("postgres"), @r"
        Ok(
            Postgres,
        )
        ");

        assert_debug_snapshot!(Dialect::from_str("foo"), @r"
        Err(
            VariantNotFound,
        )
        ");
    }

    // -- chrono_item_to_strftime tests --

    #[test]
    fn chrono_item_to_strftime_numerics_zero_pad() {
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Year, Pad::Zero)), @"%Y");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::YearMod100, Pad::Zero)), @"%y");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Month, Pad::Zero)), @"%m");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Day, Pad::Zero)), @"%d");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Hour, Pad::Zero)), @"%H");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Hour12, Pad::Zero)), @"%I");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Minute, Pad::Zero)), @"%M");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Second, Pad::Zero)), @"%S");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Nanosecond, Pad::Zero)), @"%f");
    }

    #[test]
    fn chrono_item_to_strftime_numerics_no_pad() {
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Day, Pad::None)), @"%-d");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Month, Pad::None)), @"%-m");
    }

    #[test]
    fn chrono_item_to_strftime_numerics_space_pad() {
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Day, Pad::Space)), @"%_d");
        assert_snapshot!(chrono_item_to_strftime(&Item::Numeric(Numeric::Hour, Pad::Space)), @"%_H");
    }

    #[test]
    fn chrono_item_to_strftime_numeric_unknown() {
        // Numeric variants not in the explicit list fall through to Debug format
        let result = chrono_item_to_strftime(&Item::Numeric(Numeric::Ordinal, Pad::Zero));
        assert!(result.contains("Ordinal"), "got: {result}");
    }

    #[test]
    fn chrono_item_to_strftime_fixed() {
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::ShortMonthName)), @"%b");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::LongMonthName)), @"%B");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::ShortWeekdayName)), @"%a");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::LongWeekdayName)), @"%A");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::UpperAmPm)), @"%p");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::LowerAmPm)), @"%P");
        assert_snapshot!(chrono_item_to_strftime(&Item::Fixed(Fixed::RFC3339)), @"%+");
    }

    #[test]
    fn chrono_item_to_strftime_fixed_unknown() {
        // Fixed variants not in the explicit list fall through to Debug format
        let result = chrono_item_to_strftime(&Item::Fixed(Fixed::TimezoneOffsetColon));
        assert!(result.contains("TimezoneOffsetColon"), "got: {result}");
    }

    #[test]
    fn chrono_item_to_strftime_non_numeric_non_fixed() {
        // Literal and Space items fall through to Debug format
        let result = chrono_item_to_strftime(&Item::Literal("-"));
        assert!(result.contains("Literal"), "got: {result}");

        let result = chrono_item_to_strftime(&Item::Space(" "));
        assert!(result.contains("Space"), "got: {result}");
    }

    // -- BigQueryDialect::translate_chrono_item tests --

    #[test]
    fn bigquery_translate_numeric_specifiers() {
        let bq = BigQueryDialect;
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Year, Pad::Zero)).unwrap(), @"%Y");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::YearMod100, Pad::Zero)).unwrap(), @"%y");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Month, Pad::Zero)).unwrap(), @"%m");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Day, Pad::Zero)).unwrap(), @"%d");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Hour, Pad::Zero)).unwrap(), @"%H");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Hour12, Pad::Zero)).unwrap(), @"%I");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Minute, Pad::Zero)).unwrap(), @"%M");
        assert_snapshot!(bq.translate_chrono_item(Item::Numeric(Numeric::Second, Pad::Zero)).unwrap(), @"%S");
    }

    #[test]
    fn bigquery_translate_fixed_specifiers() {
        let bq = BigQueryDialect;
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::ShortMonthName)).unwrap(), @"%b");
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::LongMonthName)).unwrap(), @"%B");
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::ShortWeekdayName)).unwrap(), @"%a");
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::LongWeekdayName)).unwrap(), @"%A");
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::UpperAmPm)).unwrap(), @"%p");
        assert_snapshot!(bq.translate_chrono_item(Item::Fixed(Fixed::RFC3339)).unwrap(), @"%Y-%m-%dT%H:%M:%S%Ez");
    }

    #[test]
    fn bigquery_translate_literal() {
        let bq = BigQueryDialect;
        assert_snapshot!(bq.translate_chrono_item(Item::Literal("-")).unwrap(), @"-");
        assert_snapshot!(bq.translate_chrono_item(Item::Literal("/")).unwrap(), @"/");
        // Single quotes are escaped by doubling
        assert_snapshot!(bq.translate_chrono_item(Item::Literal("'")).unwrap(), @"''");
        // Percent signs are escaped by doubling
        assert_snapshot!(bq.translate_chrono_item(Item::Literal("%")).unwrap(), @"%%");
    }

    #[test]
    fn bigquery_translate_space() {
        let bq = BigQueryDialect;
        assert_snapshot!(bq.translate_chrono_item(Item::Space(" ")).unwrap(), @" ");
        assert_snapshot!(bq.translate_chrono_item(Item::Space("  ")).unwrap(), @"  ");
    }

    #[test]
    fn bigquery_translate_unsupported_specifier() {
        let bq = BigQueryDialect;

        // Nanosecond (%f) is not supported by BigQuery
        let err = bq
            .translate_chrono_item(Item::Numeric(Numeric::Nanosecond, Pad::Zero))
            .unwrap_err();
        assert_snapshot!(err.to_string(), @r#"Error { kind: Error, span: None, reason: Simple("format specifier `%f` is not supported for BigQuery"), hints: [], code: None }"#);

        // Non-zero padding is not supported by BigQuery
        let err = bq
            .translate_chrono_item(Item::Numeric(Numeric::Day, Pad::None))
            .unwrap_err();
        assert_snapshot!(err.to_string(), @r#"Error { kind: Error, span: None, reason: Simple("format specifier `%-d` is not supported for BigQuery"), hints: [], code: None }"#);
        let err = bq
            .translate_chrono_item(Item::Numeric(Numeric::Month, Pad::None))
            .unwrap_err();
        assert_snapshot!(err.to_string(), @r#"Error { kind: Error, span: None, reason: Simple("format specifier `%-m` is not supported for BigQuery"), hints: [], code: None }"#);

        // LowerAmPm (%P) is not supported by BigQuery
        let err = bq
            .translate_chrono_item(Item::Fixed(Fixed::LowerAmPm))
            .unwrap_err();
        assert_snapshot!(err.to_string(), @r#"Error { kind: Error, span: None, reason: Simple("format specifier `%P` is not supported for BigQuery"), hints: [], code: None }"#);
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
