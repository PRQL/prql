mod ast;
mod ast_fold;
mod cli;
mod materializer;
mod parser;
mod translator;
mod utils;

pub use anyhow::Result; // TODO: create an error type for prql and export here
pub use cli::Cli;
pub use materializer::materialize;
pub use parser::parse;
pub use translator::translate;

/// Convert a PRQL string to SQL string. This happens in three stages:
/// - Parsing — take a string and turn it into an AST.
/// — Materialization — take an AST direct from the PRQL string and materialize
///   it into an explicit / flat AST by running functions, repacing variables, etc.
/// — Translation — take an AST and turn it into SQL.
pub fn transpile(prql: &str) -> Result<String> {
    parse(prql)
        .and_then(materialize)
        .and_then(|x| translate(&x))
}
