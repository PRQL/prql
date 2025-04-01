use super::lr::Token;
use crate::error::{Error, Reason};

/// Placeholder for chumsky 0.10 implementation
/// This is a stub that will be implemented in the future
pub fn lex_source_recovery(_source: &str, _source_id: u16) -> (Option<Vec<Token>>, Vec<Error>) {
    log::error!("Chumsky 0.10 lexer is not yet implemented");
    (None, vec![Error::new(Reason::Internal {
        message: "Chumsky 0.10 lexer is not yet implemented".to_string(),
    })])
}

/// Placeholder for chumsky 0.10 implementation
/// This is a stub that will be implemented in the future
pub fn lex_source(_source: &str) -> Result<super::lr::Tokens, Vec<Error>> {
    Err(vec![Error::new(Reason::Internal {
        message: "Chumsky 0.10 lexer is not yet implemented".to_string(),
    })])
}