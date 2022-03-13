mod ast;
mod ast_fold;
mod cli;
mod compiler;
mod parser;
mod to_sql;
mod utils;

pub use anyhow::Result; // TODO: create an error type for prql and export here
pub use cli::Cli;
pub use compiler::compile;
pub use parser::parse;
pub use to_sql::sql_of_ast;

/// Convert PRQL string to SQL string.
pub fn sql_of_prql(prql: &str) -> Result<String> {
    parse(prql).and_then(compile).and_then(|x| sql_of_ast(&x))
}
