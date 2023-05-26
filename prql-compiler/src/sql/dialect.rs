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
use sqlparser::ast::{self as sql_ast, Function, FunctionArg, FunctionArgExpr, ObjectName};
use std::any::{Any, TypeId};
use strum::VariantNames;

use crate::Error;

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
    Generic,
    Hive,
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
            Dialect::Ansi | Dialect::Generic | Dialect::Hive => Box::new(GenericDialect),
        }
    }

    #[deprecated(note = "Use `Dialect::Variants` instead")]
    pub fn names() -> &'static [&'static str] {
        Dialect::VARIANTS
    }
}

impl Default for Dialect {
    fn default() -> Self {
        Dialect::Generic
    }
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

    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_function(search, target, "REGEXP")
    }

    fn translate_regex_with_function(
        // This `self` isn't actually used, but it's require because of object
        // safety (open to better ways of doing this...)
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
        function_name: &str,
    ) -> anyhow::Result<sql_ast::Expr> {
        let args = [search, target]
            .into_iter()
            .map(FunctionArgExpr::Expr)
            .map(FunctionArg::Unnamed)
            .collect();

        Ok(sql_ast::Expr::Function(Function {
            name: ObjectName(vec![sql_ast::Ident::new(function_name)]),
            args,
            over: None,
            distinct: false,
            special: false,
            order_by: vec![],
        }))
    }

    fn translate_regex_with_operator(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
        operator: sql_ast::BinaryOperator,
    ) -> anyhow::Result<sql_ast::Expr> {
        Ok(sql_ast::Expr::BinaryOp {
            left: Box::new(search),
            op: operator,
            right: Box::new(target),
        })
    }
}

impl dyn DialectHandler {
    #[inline]
    pub fn is<T: DialectHandler + 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}

impl DialectHandler for GenericDialect {}

impl DialectHandler for PostgresDialect {
    fn requires_quotes_intervals(&self) -> bool {
        true
    }
    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_operator(search, target, sql_ast::BinaryOperator::PGRegexMatch)
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

    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_operator(
            search,
            target,
            sql_ast::BinaryOperator::Custom("REGEXP".to_string()),
        )
    }
}

impl DialectHandler for MsSqlDialect {
    fn use_top(&self) -> bool {
        true
    }

    fn translate_regex(
        &self,
        _search: sql_ast::Expr,
        _target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        Err(Error::new(crate::Reason::Simple(
            "regex functions are not supported by MsSql".to_string(),
        )))?
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

    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_operator(
            search,
            target,
            sql_ast::BinaryOperator::Custom("REGEXP".to_string()),
        )
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
    fn column_exclude(&self) -> Option<ColumnExclude> {
        // https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#select_except
        Some(ColumnExclude::Except)
    }

    fn set_ops_distinct(&self) -> bool {
        // https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#set_operators
        true
    }

    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_function(search, target, "REGEXP_CONTAINS")
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

    fn translate_regex(
        &self,
        search: sql_ast::Expr,
        target: sql_ast::Expr,
    ) -> anyhow::Result<sql_ast::Expr> {
        self.translate_regex_with_function(search, target, "REGEXP_MATCHES")
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
