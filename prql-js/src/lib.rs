// https://github.com/rustwasm/wasm-bindgen/pull/2984
#![allow(clippy::drop_non_drop)]
mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> Option<String> {
    return_or_throw(
        Ok(prql_query)
            .and_then(prql_compiler::pl_of_prql)
            .and_then(prql_compiler::rq_of_pl)
            .and_then(|rq| {
                prql_compiler::sql_of_rq(rq, options.map(prql_compiler::sql::Options::from))
            })
            .map_err(|e| e.composed("", prql_query, false)),
    )
}

#[wasm_bindgen]
pub fn pl_of_prql(prql_query: &str) -> Option<String> {
    return_or_throw(
        Ok(prql_query)
            .and_then(prql_compiler::pl_of_prql)
            .and_then(prql_compiler::json_of_pl),
    )
}

#[wasm_bindgen]
pub fn rq_of_pl(pl_json: &str) -> Option<String> {
    return_or_throw(
        Ok(pl_json)
            .and_then(prql_compiler::pl_of_json)
            .and_then(prql_compiler::rq_of_pl)
            .and_then(prql_compiler::json_of_rq),
    )
}

#[wasm_bindgen]
pub fn sql_of_rq(rq_json: &str) -> Option<String> {
    return_or_throw(
        Ok(rq_json)
            .and_then(prql_compiler::rq_of_json)
            .and_then(|x| prql_compiler::sql_of_rq(x, None)),
    )
}

/// Compilation options for SQL backend of the compiler.
#[wasm_bindgen]
#[derive(Clone)]
pub struct CompileOptions {
    /// True for passing generated SQL string trough a formatter that splits
    /// into multiple lines and prettifies indentation and spacing.
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
}

#[wasm_bindgen]
pub fn default_compile_options() -> CompileOptions {
    CompileOptions {
        format: true,
        dialect: None,
    }
}

impl From<CompileOptions> for prql_compiler::sql::Options {
    fn from(o: CompileOptions) -> Self {
        prql_compiler::sql::Options {
            format: o.format,
            dialect: o.dialect.map(From::from),
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
        }
    }
}

fn return_or_throw(result: Result<String, prql_compiler::ErrorMessages>) -> Option<String> {
    match result {
        Ok(sql) => Some(sql),
        Err(e) => wasm_bindgen::throw_str(&e.to_json()),
    }
}
