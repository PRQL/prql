mod ast;
mod ast_fold;
#[cfg(feature = "cli")]
mod cli;
mod error;
mod parser;
mod semantic;
mod translator;
mod utils;

pub use anyhow::Result;
#[cfg(feature = "cli")]
pub use cli::Cli;
pub use error::{format_error, SourceLocation};
pub use parser::parse;
pub use translator::translate;

/// Compile a PRQL string into a SQL string.
///
/// This has two stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [translate] — Write a SQL string from a PRQL AST.
pub fn compile(prql: &str) -> Result<String, (String, Option<SourceLocation>)> {
    parse(prql)
        .and_then(|x| translate(&x))
        .map_err(|e| format_error(e, "", prql))
}

/// Exposes some library internals.
///
/// They are primarily exposed for documentation. There may be issues with using
/// the exported items without items they rely on — feel free to request
/// associated items be made public if required.
pub mod internals {
    pub use crate::ast::Node;
    pub use crate::ast_fold::AstFold;
    pub use crate::utils::{IntoOnly, Only};
}
