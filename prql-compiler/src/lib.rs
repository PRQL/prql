//! Compiler for PRQL language.
//! Targets SQL and exposes PL and RQ abstract syntax trees.
//!
//! You probably want to start with [compile] wrapper function.
//!
//! For more granular access, refer to this diagram:
//! ```ascii
//!            PRQL
//!
//!    (parse) │ ▲
//! pl_of_prql │ │ prql_of_pl
//!            │ │
//!            ▼ │      pl_of_json
//!                   ────────►
//!           PL AST            PL JSON
//!                   ◄────────
//!            │        json_of_pl
//!            │
//!  (resolve) │
//!   rq_of_pl │
//!            │
//!            │
//!            ▼        rq_of_json
//!                   ────────►
//!           RQ AST            RQ JSON
//!                   ◄────────
//!            │        json_of_rq
//!            │
//!  sql_of_rq │
//!            ▼
//!
//!            SQL
//! ```

// Our error type is 128 bytes, because it contains 5 strings & an Enum, which
// is exactly the default warning level. Given we're not that performance
// sensitive, it's fine to ignore this at the moment (and not worth having a
// clippy config file for a single setting). We can consider adjusting it as a
// yak-shaving execise in the future.
#![allow(clippy::result_large_err)]

pub mod ast;
#[cfg(all(feature = "cli", not(target_family = "wasm")))]
mod cli;
mod error;
mod parser;
pub mod semantic;
pub mod sql;
#[cfg(test)]
mod test;
mod utils;

#[cfg(all(feature = "cli", not(target_family = "wasm")))]
pub use cli::Cli;
pub use error::{ErrorMessage, ErrorMessages, SourceLocation};
pub use utils::IntoOnly;

use once_cell::sync::Lazy;
use semver::Version;

static PRQL_VERSION: Lazy<Version> =
    Lazy::new(|| Version::parse(env!("CARGO_PKG_VERSION")).expect("Invalid PRQL version number"));

/// Compile a PRQL string into a SQL string.
///
/// This is a wrapper for:
/// - [pl_of_prql] — Build PL AST from a PRQL string
/// - [rq_of_pl] — Finds variable references, validates functions calls, determines frames and converts PL to RQ.
/// - [sql_of_rq] — Convert RQ AST into an SQL string.
pub fn compile(prql: &str) -> Result<String, ErrorMessages> {
    parser::parse(prql)
        .and_then(semantic::resolve)
        .and_then(|rq| sql::compile(rq, None))
        .map_err(error::downcast)
        .map_err(|e| e.composed("", prql, false))
}

/// Parse PRQL into a PL AST
pub fn pl_of_prql(prql: &str) -> Result<Vec<ast::pl::Stmt>, ErrorMessages> {
    parser::parse(prql)
        .map_err(error::downcast)
        .map_err(|e| e.composed("", prql, false))
}

/// Perform semantic analysis and convert PL to RQ.
pub fn rq_of_pl(pl: Vec<ast::pl::Stmt>) -> Result<ast::rq::Query, ErrorMessages> {
    semantic::resolve(pl).map_err(error::downcast)
}

/// Generate SQL from RQ.
pub fn sql_of_rq(
    rq: ast::rq::Query,
    options: Option<sql::Options>,
) -> Result<String, ErrorMessages> {
    sql::compile(rq, options).map_err(error::downcast)
}

/// Generate PRQL code from PL AST
pub fn prql_of_pl(pl: Vec<ast::pl::Stmt>) -> Result<String, ErrorMessages> {
    Ok(format!("{}", ast::pl::Statements(pl)))
}

/// JSON serialization
pub fn json_of_pl(pl: Vec<ast::pl::Stmt>) -> Result<String, ErrorMessages> {
    serde_json::to_string(&pl).map_err(|e| error::downcast(anyhow::anyhow!(e)))
}

/// JSON deserialization
pub fn pl_of_json(json: &str) -> Result<Vec<ast::pl::Stmt>, ErrorMessages> {
    serde_json::from_str(json).map_err(|e| error::downcast(anyhow::anyhow!(e)))
}

/// JSON deserialization
pub fn rq_of_json(json: &str) -> Result<ast::rq::Query, ErrorMessages> {
    serde_json::from_str(json).map_err(|e| error::downcast(anyhow::anyhow!(e)))
}

/// JSON serialization
pub fn json_of_rq(rq: ast::rq::Query) -> Result<String, ErrorMessages> {
    serde_json::to_string(&rq).map_err(|e| error::downcast(anyhow::anyhow!(e)))
}
