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

/// Convert PRQL string to SQL string.
pub fn transpile(prql: &str) -> Result<String> {
    parse(prql)
        .and_then(materialize)
        .and_then(|x| translate(&x))
}
