// TODO: exporting `foo::*` and exporting the module `foo` seems prone to
// confusion and IIUC serves no purpose — IIUC it guarantees there are two ways
// of accessing the same variable

pub mod expr;
pub mod ident;
pub mod ops;
pub mod stmt;
pub mod types;

pub use expr::*;
pub use ident::*;
pub use ops::*;
pub use stmt::*;
pub use types::*;

// re-export Literal from LR, since it's encapsulated in TyKind

pub use crate::lexer::lr::Literal;
pub use crate::lexer::lr::ValueAndUnit;
pub use crate::parser::generic;
pub use crate::span::Span;
