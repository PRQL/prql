mod chumsky_0_10;

pub mod lr;
#[cfg(test)]
mod test;

// Re-export the Chumsky 0.10 implementation
pub use chumsky_0_10::{lex_source, lex_source_recovery};

// Testing helper for debugging the lexer
#[cfg(test)]
pub mod debug {
    use super::*;

    pub fn lex_debug(source: &str) -> Result<lr::Tokens, Vec<crate::error::Error>> {
        chumsky_0_10::lex_source(source)
    }
}
