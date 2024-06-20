pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
mod test;

pub use self::lexer::TokenVec;
use crate::error::Error;
use crate::lexer::lr::TokenKind;
use crate::parser::pr::Stmt;

/// Build PRQL AST from a PRQL query string.
pub fn parse_source(source: &str, source_id: u16) -> Result<Vec<Stmt>, Vec<Error>> {
    let mut errors = Vec::new();

    let (tokens, lex_errors) = ::chumsky::Parser::parse_recovery(&lexer::lexer(), source);

    log::debug!("Lex errors: {:?}", lex_errors);
    errors.extend(
        lex_errors
            .into_iter()
            .map(|e| lexer::convert_lexer_error(source, e, source_id)),
    );

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
        let stream = parser::prepare_stream(semantic_tokens, source, source_id);

        let (ast, parse_errors) =
            ::chumsky::Parser::parse_recovery(&parser::stmt::source(), stream);

        log::debug!("parse errors: {:?}", parse_errors);
        errors.extend(parse_errors.into_iter().map(|e| e.into()));

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
