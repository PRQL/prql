// These bindings aren't relevant on wasm
#![cfg(not(target_family = "wasm"))]
// TODO: unclear why we need this `allow`; it's required in `CompileOptions`,
// likely because of the `NifStruct` derive.
#![allow(clippy::needless_borrow)]

use std::default::Default;

use rustler::{Atom, NifResult, NifStruct, NifTuple};

mod atoms {
    rustler::atoms! {
      ok,
      error,

      // dialects
      ansi,
      bigquery,
      clickhouse,
      glaredb,
      generic,
      mssql,
      mysql,
      postgres,
      sqlite,
      snowflake
    }
}

/// Convert a `Result` from PRQL into a tuple in elixir `{:ok, binary()} | {:error, binary()}`
fn to_result_tuple(result: Result<String, prqlc::ErrorMessages>) -> NifResult<Response> {
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
fn target_from_atom(a: Atom) -> prqlc::Target {
    use prqlc::sql::Dialect::*;

    prqlc::Target::Sql(Some(if a == atoms::ansi() {
        Ansi
    } else if a == atoms::bigquery() {
        BigQuery
    } else if a == atoms::clickhouse() {
        ClickHouse
    } else if a == atoms::generic() {
        Generic
    } else if a == atoms::glaredb() {
        GlareDb
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

impl From<CompileOptions> for prqlc::Options {
    /// Get `prqlc::Options` options from `CompileOptions`
    fn from(o: CompileOptions) -> Self {
        prqlc::Options {
            format: o.format,
            target: target_from_atom(o.target),
            signature_comment: o.signature_comment,
            display: prqlc::DisplayOptions::Plain,
            ..Default::default()
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
    /// If it does not exist, [`prqlc::sql::Dialect::Generic`] is used.
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
    to_result_tuple(prqlc::compile(prql_query, &options.into()))
}

#[rustler::nif]
/// convert a prql query into PL AST
pub fn prql_to_pl(prql_query: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(prql_query)
            .and_then(prqlc::prql_to_pl)
            .and_then(|x| prqlc::json::from_pl(&x)),
    )
}

#[rustler::nif]
/// Convert PL AST into RQ
pub fn pl_to_rq(pl_json: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(pl_json)
            .and_then(prqlc::json::to_pl)
            .and_then(prqlc::pl_to_rq)
            .and_then(|x| prqlc::json::from_rq(&x)),
    )
}

#[rustler::nif]
/// Convert RQ to SQL
pub fn rq_to_sql(rq_json: &str) -> NifResult<Response> {
    to_result_tuple(
        Ok(rq_json)
            .and_then(prqlc::json::to_rq)
            // Currently just using default options here; probably should pass
            // an argument from this func.
            .and_then(|x| prqlc::rq_to_sql(x, &prqlc::Options::default())),
    )
}

rustler::init!("Elixir.PRQL.Native");
