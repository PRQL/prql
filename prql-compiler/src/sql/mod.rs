//! Backend for translating RQ into SQL

mod anchor;
mod codegen;
mod context;
mod target;
mod preprocess;
mod std;
mod translator;

pub use target::Target;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{ast::rq::Query, PRQL_VERSION};

/// Translate a PRQL AST into a SQL string.
pub fn compile(query: Query, options: Option<Options>) -> Result<String> {
    let options = options.unwrap_or_default();

    let sql_ast = translator::translate_query(query, options.target)?;

    let sql = sql_ast.to_string();

    // formatting
    let sql = if options.format {
        let formatted = sqlformat::format(
            &sql,
            &sqlformat::QueryParams::default(),
            sqlformat::FormatOptions::default(),
        );

        // The sql formatter turns `{{` into `{ {`, and while that's reasonable SQL,
        // we want to allow jinja expressions through. So we (somewhat hackily) replace
        // any `{ {` with `{{`.
        formatted.replace("{ {", "{{").replace("} }", "}}")
    } else {
        sql
    };

    // signature
    let sql = if options.signature_comment {
        let pre = if options.format { "\n\n" } else { " " };
        let post = if options.format { "\n" } else { "" };
        let signature = format!(
            "{pre}-- Generated by PRQL compiler version {} (https://prql-lang.org){post}",
            *PRQL_VERSION
        );
        sql + &signature
    } else {
        sql
    };

    Ok(sql)
}

/// Compilation options for SQL backend of the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target to compile to (generally a SQL dialect).
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic target may work with most
    /// databases.
    ///
    /// If something does not work in the target / dialect you need, please
    /// report it at GitHub issues.
    ///
    /// If None is used, `target` flag from query definition is used. If it does
    /// not exist, [Target::Generic] is used.
    pub target: Option<Target>,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: true,
            target: None,
            signature_comment: true,
        }
    }
}

impl Options {
    pub fn no_format(mut self) -> Self {
        self.format = false;
        self
    }

    pub fn no_signature(mut self) -> Self {
        self.signature_comment = false;
        self
    }

    pub fn with_target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }

    pub fn some(self) -> Option<Self> {
        Some(self)
    }
}
