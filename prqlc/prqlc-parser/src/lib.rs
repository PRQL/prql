pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
#[cfg(test)]
mod test;

pub use self::lexer::TokenVec;
use crate::error::Error;
use crate::lexer::lr::TokenKind;
use crate::parser::pr::Stmt;

/// Build PRQL AST from a PRQL query string.
pub fn parse_source(source: &str, source_id: u16) -> Result<Vec<Stmt>, Vec<Error>> {
    let (tokens, mut errors) = lexer::lex_string_recovery(source, source_id);
    log::debug!("Lex errors: {:?}", errors);

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

    let ast = if let Some(semantic_tokens) = semantic_tokens {
        let (ast, parse_errors) = parser::parse_lr_to_pr(source, source_id, semantic_tokens);

        log::debug!("parse errors: {:?}", parse_errors);
        errors.extend(parse_errors);

        ast
    } else {
        None
    };

    if errors.is_empty() {
        Ok(ast.unwrap_or_default())
    } else {
        Err(errors)
    }
}
