mod ast;
mod ast_fold;
#[cfg(feature = "cli")]
mod cli;
mod materializer;
mod parser;
mod translator;
mod utils;

pub use anyhow::Result; // TODO: create an error type for prql and export here
#[cfg(feature = "cli")]
pub use cli::Cli;
pub use materializer::materialize;
pub use parser::parse;
pub use translator::translate;

/// Compile a PRQL string into a SQL string.
///
/// This has three stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [materialize] — "Flatten" a PRQL AST by running functions & replacing variables.
/// - [translate] — Write a SQL string from a PRQL AST.
pub fn compile(prql: &str) -> Result<String> {
    parse(prql)
        .and_then(materialize)
        .and_then(|x| translate(&x))
}

/// Exposes some library internals.
///
/// They are primarily exposed for documentation. There may be issues with using
/// the exported items without items they rely on — feel free to request
/// associated items be made public if required.
pub mod internals {
    pub use crate::ast::Item;
    pub use crate::ast_fold::AstFold;
    pub use crate::utils::{IntoOnly, Only};
    pub use anyhow::Result; // TODO: create an error type for prql and export here
}
