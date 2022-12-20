//! Backend for translating RQ into SQL

mod anchor;
mod codegen;
mod context;
mod dialect;
mod preprocess;
mod translator;

pub use dialect::Dialect;
use serde::{Deserialize, Serialize};
pub use translator::compile;

/// Compilation options for SQL backend of the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// True for passing generated SQL string trough a formatter that splits
    /// into multiple lines and prettifies indentation and spacing.
    pub format: bool,

    /// Target dialect you want to compile for.
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic dialect may work with most
    /// databases.
    ///
    /// If something does not work in dialect you need, please report it at
    /// GitHub issues.
    ///
    /// If None is used, `sql_dialect` flag from query definition is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub dialect: Option<Dialect>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: true,
            dialect: None,
        }
    }
}
