pub mod ast;
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
    parse(prql).and_then(resolve_and_translate)
}

/// Format an PRQL query
///
/// This has two stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [display] — Write a AST back to string.
pub fn format(prql: &str) -> Result<String> {
    parse(prql).map(display)
}

/// Compile a PRQL string into a JSON version of the Query.
pub fn to_json(prql: &str) -> Result<String> {
    Ok(serde_json::to_string(&parse(prql)?)?)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::to_json;

    #[test]
    fn test_to_json() -> Result<()> {
        let json = to_json("from employees | take 10")?;
        // Since the AST is so in flux right now just test that the brackets are present
        assert_eq!(json.chars().next().unwrap(), '{');
        assert_eq!(json.chars().nth(json.len() - 1).unwrap(), '}');

        Ok(())
    }
}
