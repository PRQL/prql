// https://github.com/rustwasm/wasm-bindgen/pull/2984
#![allow(clippy::drop_non_drop)]
mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile(prql_query: &str, options: Option<SQLCompileOptions>) -> Option<String> {
    return_or_throw(
        prql_compiler::compile(prql_query, options.map(|x| x.into()))
            .map_err(|e| e.composed("", prql_query, false)),
    )
}

#[wasm_bindgen]
pub fn prql_to_pl(prql_query: &str) -> Option<String> {
    return_or_throw(
        Ok(prql_query)
            .and_then(prql_compiler::prql_to_pl)
            .and_then(prql_compiler::json::from_pl),
    )
}

#[wasm_bindgen]
pub fn pl_to_rq(pl_json: &str) -> Option<String> {
    return_or_throw(
        Ok(pl_json)
            .and_then(prql_compiler::json::to_pl)
            .and_then(prql_compiler::pl_to_rq)
            .and_then(prql_compiler::json::from_rq),
    )
}

#[wasm_bindgen]
pub fn rq_to_sql(rq_json: &str) -> Option<String> {
    return_or_throw(
        Ok(rq_json)
            .and_then(prql_compiler::json::to_rq)
            .and_then(|x| prql_compiler::rq_to_sql(x, None)),
    )
}

/// Compilation options for SQL backend of the compiler.
#[wasm_bindgen]
#[derive(Clone)]
pub struct SQLCompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target dialect you want to compile for.
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic dialect may work with most
    /// databases.
    ///
    /// If something does not work in dialect you need, please report it at
    /// GitHub issues.
    ///
    /// If None is used, `sql_dialect` flag from query definition is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub dialect: Option<Dialect>,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

impl Default for SQLCompileOptions {
    fn default() -> Self {
        Self {
            format: true,
            dialect: None,
            signature_comment: true,
        }
    }
}

#[wasm_bindgen]
pub fn default_compile_options() -> SQLCompileOptions {
    SQLCompileOptions::default()
}

#[wasm_bindgen]
impl SQLCompileOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<SQLCompileOptions> for prql_compiler::sql::Options {
    fn from(o: SQLCompileOptions) -> Self {
        prql_compiler::sql::Options {
            format: o.format,
            dialect: o.dialect.map(From::from),
            signature_comment: o.signature_comment,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Dialect {
    Ansi,
    BigQuery,
    ClickHouse,
    Generic,
    Hive,
    MsSql,
    MySql,
    PostgreSql,
    SQLite,
    Snowflake,
    DuckDb,
}

impl From<Dialect> for prql_compiler::sql::Dialect {
    fn from(d: Dialect) -> Self {
        use prql_compiler::sql::Dialect as D;
        match d {
            Dialect::Ansi => D::Ansi,
            Dialect::BigQuery => D::BigQuery,
            Dialect::ClickHouse => D::ClickHouse,
            Dialect::Generic => D::Generic,
            Dialect::Hive => D::Hive,
            Dialect::MsSql => D::MsSql,
            Dialect::MySql => D::MySql,
            Dialect::PostgreSql => D::PostgreSql,
            Dialect::SQLite => D::SQLite,
            Dialect::Snowflake => D::Snowflake,
            Dialect::DuckDb => D::DuckDb,
        }
    }
}

fn return_or_throw(result: Result<String, prql_compiler::ErrorMessages>) -> Option<String> {
    match result {
        Ok(sql) => Some(sql),
        Err(e) => wasm_bindgen::throw_str(&e.to_json()),
    }
}
