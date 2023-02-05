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
      hive,
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

/// Get the dialect from an atom. By default `Generic` dialect will be used
fn dialect_from_atom(a: Atom) -> prql_compiler::sql::Dialect {
    use prql_compiler::sql::Dialect as D;

    if a == atoms::ansi() {
        D::Ansi
    } else if a == atoms::bigquery() {
        D::BigQuery
    } else if a == atoms::clickhouse() {
        D::ClickHouse
    } else if a == atoms::generic() {
        D::Generic
    } else if a == atoms::hive() {
        D::Hive
    } else if a == atoms::mssql() {
        D::MsSql
    } else if a == atoms::mysql() {
        D::MySql
    } else if a == atoms::postgres() {
        D::PostgreSql
    } else if a == atoms::sqlite() {
        D::SQLite
    } else if a == atoms::snowflake() {
        D::Snowflake
    } else {
        D::Generic
    }
}

impl From<CompileOptions> for prql_compiler::sql::Options {
    /// Get `prql_compiler::sql::Options` options from `CompileOptions`
    fn from(o: CompileOptions) -> Self {
        prql_compiler::sql::Options {
            format: o.format,
            dialect: Some(dialect_from_atom(o.dialect)),
            signature_comment: o.signature_comment,
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
    pub dialect: Atom,

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
    to_result_tuple(prql_compiler::compile(prql_query, Some(options.into())))
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
            .and_then(|x| prql_compiler::rq_to_sql(x, None)),
    )
}

rustler::init!(
    "Elixir.PRQL.Native",
    [compile, prql_to_pl, pl_to_rq, rq_to_sql]
);
