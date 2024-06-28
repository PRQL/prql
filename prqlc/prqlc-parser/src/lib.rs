pub mod error;
pub mod generic;
pub mod lexer;
pub mod parser;
pub mod span;
#[cfg(test)]
mod test;

use crate::lexer::lr::TokenKind;
