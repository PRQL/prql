// https://github.com/rustwasm/wasm-bindgen/pull/2984
#![allow(clippy::drop_non_drop)]
mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> Option<String> {
    return_or_throw(
        Ok(prql_query)
            .and_then(prql_compiler::prql_to_pl)
            .and_then(prql_compiler::pl_to_rq)
            .and_then(|rq| {
                prql_compiler::rq_to_sql(rq, options.map(prql_compiler::sql::Options::from))
            })
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

// TODO: `CompileOptions` is replicated in `prql-compiler/src/sql/mod.rs`; can
// we combine them despite the `wasm_bindgen` attribute?

/// Compilation options for SQL backend of the compiler.
#[wasm_bindgen]
#[derive(Clone)]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target to compile to (generally a SQL dialect).
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic target may work with most
    /// databases.
    ///
    /// If something does not work in the target / dialect you need, please
    /// report it at GitHub issues.
    ///
    /// If None is used, `target` flag from query definition is used. If it does
    /// not exist, [Target::Generic] is used.
    pub target: Option<Target>,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

#[wasm_bindgen]
pub fn default_compile_options() -> CompileOptions {
    CompileOptions {
        format: true,
        target: None,
        signature_comment: true,
    }
}

impl From<CompileOptions> for prql_compiler::sql::Options {
    fn from(o: CompileOptions) -> Self {
        prql_compiler::sql::Options {
            format: o.format,
            target: o.target.map(From::from),
            signature_comment: o.signature_comment,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Target {
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

impl From<Target> for prql_compiler::sql::Target {
    fn from(d: Target) -> Self {
        use prql_compiler::sql::Target as D;
        match d {
            Target::Ansi => D::Ansi,
            Target::BigQuery => D::BigQuery,
            Target::ClickHouse => D::ClickHouse,
            Target::Generic => D::Generic,
            Target::Hive => D::Hive,
            Target::MsSql => D::MsSql,
            Target::MySql => D::MySql,
            Target::PostgreSql => D::PostgreSql,
            Target::SQLite => D::SQLite,
            Target::Snowflake => D::Snowflake,
        }
    }
}

fn return_or_throw(result: Result<String, prql_compiler::ErrorMessages>) -> Option<String> {
    match result {
        Ok(sql) => Some(sql),
        Err(e) => wasm_bindgen::throw_str(&e.to_json()),
    }
}
