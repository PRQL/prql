mod lexer;

pub mod lr;
#[cfg(test)]
mod test;

pub use lexer::{lex_source, lex_source_recovery};

#[cfg(test)]
pub mod debug {
    use super::*;

    pub fn lex_debug(source: &str) -> Result<lr::Tokens, Vec<crate::error::Error>> {
        lexer::lex_source(source)
    }
}
