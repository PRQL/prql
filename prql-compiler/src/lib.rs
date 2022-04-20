mod ast;
#[cfg(feature = "cli")]
mod cli;
mod error;
mod parser;
mod semantic;
mod sql;
mod utils;

pub use anyhow::Result;
pub use ast::display;
#[cfg(feature = "cli")]
pub use cli::Cli;
pub use error::{format_error, SourceLocation};
pub use parser::parse;
pub use semantic::*;
pub use sql::{resolve_and_translate, translate};

/// Compile a PRQL string into a SQL string.
///
/// This has three stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [resolve] — Finds variable references, validates functions calls, determines frames.
/// - [translate] — Write a SQL string from a PRQL AST.
pub fn compile(prql: &str) -> Result<String> {
    parse(prql).and_then(|x| resolve_and_translate(x))
}

/// Format an PRQL query
///
/// This has two stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [display] — Write a AST back to string.
pub fn format(prql: &str) -> Result<String> {
    parse(prql).map(display)
}

/// Exposes some library internals.
///
/// They are primarily exposed for documentation. There may be issues with using
/// the exported items without items they rely on — feel free to request
/// associated items be made public if required.
pub mod internals {
    pub use crate::ast::ast_fold::AstFold;
    pub use crate::ast::Node;
    pub use crate::utils::{IntoOnly, Only};
}
