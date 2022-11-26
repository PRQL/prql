//! Backend for translating RQ into SQL

mod anchor;
mod codegen;
mod context;
mod distinct;
mod translator;

pub use translator::translate;
