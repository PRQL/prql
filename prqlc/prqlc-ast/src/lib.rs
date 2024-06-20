pub mod expr;
pub mod span;
pub mod stmt;
pub mod token;
mod types;

pub use expr::*;
pub use span::*;
pub use stmt::*;
pub use token::*;
pub use types::*;

pub trait WithAesthetics {
    fn with_aesthetics(
        self,
        aesthetics_before: Vec<TokenKind>,
        aethetics_after: Vec<TokenKind>,
    ) -> Self;
}
