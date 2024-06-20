//this exports everything within as if it was at the root.
//eg: PR::expr::Expr becomes PR::Expr
pub use expr::*;
pub use ident::*;
pub use ops::*;
pub use stmt::*;
pub use types::*;

//re-export Literal from LR, since it's encapsulated in TyKind

pub mod expr;
pub mod stmt;

pub mod ident;
pub mod ops;
pub mod types;

pub use crate::lexer::lr::Literal;
pub use crate::lexer::lr::ValueAndUnit;
pub use crate::parser::generic;
pub use crate::span::Span;
