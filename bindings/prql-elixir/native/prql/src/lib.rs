// Seems to break tarpaulin
#![cfg(not(tarpaulin))]
// See Readme for more information on Mac compiling
#![cfg(not(target_os = "macos"))]
// These bindings aren't relevant on wasm
#![cfg(not(target_family = "wasm"))]
// TODO: unclear why we need this `allow`; it's required in `CompileOptions`,
// likely because of the `NifStruct` derive.
#![allow(clippy::needless_borrow)]

use rustler::{Atom, NifResult, NifStruct, NifTuple};

mod atoms {
    rustler::atoms! {
      ok,
      error,

      // dialects
      ansi,
      bigquery,
      clickhouse,
      generic,
      mssql,
      mysql,
      postgres,
      sqlite,
      snowflake
    }
}

/// Convert a `Result` from PRQL into a tuple in elixir `{:ok, binary()} | {:error, binary()}`
fn to_result_tuple(result: Result<String, prql_compiler::ErrorMessages>) -> NifResult<Response> {
    match result {
        Ok(sql) => Ok(Response {
            status: atoms::ok(),
            result: sql,
        }),
        Err(e) => Ok(Response {
            status: atoms::error(),
            result: e.to_json(),
        }),
    }
}

/// Get the target from an atom. By default `Generic` SQL dialect will be used
fn target_from_atom(a: Atom) -> prql_compiler::Target {
    use prql_compiler::sql::Dialect::*;

    prql_compiler::Target::Sql(Some(if a == atoms::ansi() {
        Ansi
    } else if a == atoms::bigquery() {
        BigQuery
    } else if a == atoms::clickhouse() {
        ClickHouse
    } else if a == atoms::generic() {
        Generic
    } else if a == atoms::mssql() {
        MsSql
    } else if a == atoms::mysql() {
        MySql
    } else if a == atoms::postgres() {
        Postgres
    } else if a == atoms::sqlite() {
        SQLite
    } else if a == atoms::snowflake() {
        Snowflake
    } else {
        Generic
    }))
}

impl From<CompileOptions> for prql_compiler::Options {
    /// Get `prql_compiler::Options` options from `CompileOptions`
    fn from(o: CompileOptions) -> Self {
        prql_compiler::Options {
            format: o.format,
            target: target_from_atom(o.target),
            signature_comment: o.signature_comment,
            // TODO: add support for this
            color: false,
        }
    }
}

#[derive(Clone, NifStruct, Debug)]
#[module = "PRQL.Native.CompileOptions"]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target dialect to compile to.
    ///
    /// This is only changes the output for a relatively small subset of
    /// features.
    ///
    /// If something does not work in a specific dialect, please raise in a
    /// GitHub issue.
    ///
    /// If `None` is used, the `target` argument from the query header is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub target: Atom,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

#[derive(NifTuple)]
pub struct Response {
    /// status atom `:ok` or `:error`
    status: Atom,

    /// result string
    result: String,
}

#[rustler::nif]
/// compile a prql query into sql
pub fn compile(prql_query: &str, options: CompileOptions) -> NifResult<Response> {
    to_result_tuple(prql_compiler::compile(prql_query, &options.into()))
}

#[rustler::nif]
/// convert a prql query into PL AST
pub fn prql_to_pl(prql_query: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(prql_query)
            .and_then(prql_compiler::prql_to_pl)
            .and_then(prql_compiler::json::from_pl),
    )
}

#[rustler::nif]
/// Convert PL AST into RQ
pub fn pl_to_rq(pl_json: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(pl_json)
            .and_then(prql_compiler::json::to_pl)
            .and_then(prql_compiler::pl_to_rq)
            .and_then(prql_compiler::json::from_rq),
    )
}

#[rustler::nif]
/// Convert RQ to SQL
pub fn rq_to_sql(rq_json: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(rq_json)
            .and_then(prql_compiler::json::to_rq)
            // Currently just using default options here; probably should pass
            // an argument from this func.
            .and_then(|x| prql_compiler::rq_to_sql(x, &prql_compiler::Options::default())),
    )
}

rustler::init!(
    "Elixir.PRQL.Native",
    [compile, prql_to_pl, pl_to_rq, rq_to_sql]
);
