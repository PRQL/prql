use crate::error::Error;
use crate::lexer::lr::TokenKind;
use crate::parser::parse_lr_to_pr;
use crate::parser::pr::Stmt;

pub use self::lexer::TokenVec;

pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;

mod test;
// pub use crate::err::*;

/// Build PRQL AST from a PRQL query string.
pub fn parse_source(source: &str, source_id: u16) -> Result<Vec<Stmt>, Vec<Error>> {
    let mut errors = Vec::new();

    let (tokens, lex_errors) = lexer::lex_string_recovery(source, source_id);

    log::debug!("Lex errors: {:?}", lex_errors);

    // We don't want comments in the AST (but we do intend to use them as part of
    // formatting)
    let semantic_tokens: Option<_> = tokens.map(|tokens| {
        tokens.into_iter().filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Comment(_) | TokenKind::LineWrap(_) | TokenKind::DocComment(_)
            )
        })
    });

    let pr = if let Some(semantic_tokens) = semantic_tokens {
        let (pr, parse_errors) = parse_lr_to_pr(source, source_id, semantic_tokens);
        log::debug!("parse errors: {:?}", parse_errors);
        errors.extend(parse_errors.into_iter().map(|e| e.into()));

        pr
    } else {
        None
    };

    if errors.is_empty() {
        Ok(pr.unwrap_or_default())
    } else {
        Err(errors)
    }
}
