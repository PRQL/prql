//! Compiler for PRQL language.
//! Targets SQL and exposes PL and RQ abstract syntax trees.
//!
//! You probably want to start with [compile] wrapper function.
//!
//! For more granular access, refer to this diagram:
//! ```ascii
//!            PRQL
//!
//!    (parse) │ ▲
//! prql_to_pl │ │ pl_to_prql
//!            │ │
//!            ▼ │      json::from_pl
//!                   ────────►
//!           PL AST            PL JSON
//!                   ◄────────
//!            │        json::to_pl
//!            │
//!  (resolve) │
//!   pl_to_rq │
//!            │
//!            │
//!            ▼        json::from_rq
//!                   ────────►
//!           RQ AST            RQ JSON
//!                   ◄────────
//!            │        json::to_rq
//!            │
//!  rq_to_sql │
//!            ▼
//!
//!            SQL
//! ```
//!
//! ## Common use-cases
//!
//! I want to ...
//! - ... compile PRQL queries at run time, because I cannot commit them into
//!     the source tree.
//!
//!     ```
//!     # fn main() -> Result<(), prql_compiler::ErrorMessages> {
//!     let sql = prql_compiler::compile(
//!         "from albums | select {title, artist_id}",
//!          &prql_compiler::Options::default().no_format()
//!     )?;
//!     assert_eq!(&sql[..35], "SELECT title, artist_id FROM albums");
//!     # Ok(())
//!     # }
//!     ```
//!
//! - ... compile PRQL queries to SQL at build time.
//!
//!     Use `prql-compiler-macros` crate (unreleased), which can be used like
//!     this:
//!     ```ignore
//!     let sql: &str = prql_to_sql!("from albums | select {title, artist_id}");
//!     ```
//!
//! - ... compile .prql files to .sql files at build time.
//!
//!     Call this crate from `build.rs`. See
//!     [this example project](https://github.com/PRQL/prql/tree/main/prql-compiler/examples/compile-files).
//!
//! - ... compile, format & debug PRQL from command line.
//!
//!     ```sh
//!     $ cargo install prqlc
//!     $ prqlc compile query.prql
//!     ```
//!

// Our error type is 128 bytes, because it contains 5 strings & an Enum, which
// is exactly the default warning level. Given we're not that performance
// sensitive, it's fine to ignore this at the moment (and not worth having a
// clippy config file for a single setting). We can consider adjusting it as a
// yak-shaving exercise in the future.
#![allow(clippy::result_large_err)]

pub mod ast;
mod codegen;
mod error;
mod parser;
pub mod semantic;
pub mod sql;
#[cfg(test)]
mod tests;
mod utils;

pub use error::{
    downcast, Error, ErrorMessage, ErrorMessages, MessageKind, Reason, SourceLocation, Span,
};

use once_cell::sync::Lazy;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use strum::VariantNames;
use utils::IdGenerator;

pub static COMPILER_VERSION: Lazy<Version> = Lazy::new(|| {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Invalid prql-compiler version number")
});

/// Compile a PRQL string into a SQL string.
///
/// This is a wrapper for:
/// - [prql_to_pl] — Build PL AST from a PRQL string
/// - [pl_to_rq] — Finds variable references, validates functions calls, determines frames and converts PL to RQ.
/// - [rq_to_sql] — Convert RQ AST into an SQL string.
/// # Example
/// Use the prql compiler to convert a PRQL string to SQLite dialect
///
/// ```
/// use prql_compiler::{compile, Options, Target, sql::Dialect};
///
/// let prql = "from employees | select {name,age}";
/// let opts = Options {
///     format: false,
///     target: Target::Sql(Some(Dialect::SQLite)),
///     signature_comment: false,
///     color: false,
/// };
/// let sql = compile(&prql, &opts).unwrap();
/// println!("PRQL: {}\nSQLite: {}", prql, &sql);
/// assert_eq!("SELECT name, age FROM employees", sql)
///
/// ```
/// See [`sql::Options`](sql/struct.Options.html) and [`sql::Dialect`](sql/enum.Dialect.html) for options and supported SQL dialects.
pub fn compile(prql: &str, options: &Options) -> Result<String, ErrorMessages> {
    let mut sources = SourceTree::from(prql);
    semantic::load_std_lib(&mut sources);

    parser::parse(&sources)
        .and_then(|ast| semantic::resolve_and_lower(ast, &[]))
        .and_then(|rq| sql::compile(rq, options))
        .map_err(error::downcast)
        .map_err(|e| e.composed(&prql.into(), options.color))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Target {
    /// If `None` is used, dialect is extracted from `target` query header.
    Sql(Option<sql::Dialect>),
}

impl Default for Target {
    fn default() -> Self {
        Self::Sql(None)
    }
}

impl Target {
    pub fn names() -> Vec<String> {
        let mut names = vec!["sql.any".to_string()];

        let dialects = sql::Dialect::VARIANTS;
        names.extend(dialects.iter().map(|d| format!("sql.{d}")));

        names
    }
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(s: &str) -> Result<Target, Self::Err> {
        if let Some(dialect) = s.strip_prefix("sql.") {
            if dialect == "any" {
                return Ok(Target::Sql(None));
            }

            if let Ok(dialect) = sql::Dialect::from_str(dialect) {
                return Ok(Target::Sql(Some(dialect)));
            }
        }

        Err(Error::new(Reason::NotFound {
            name: format!("{s:?}"),
            namespace: "target".to_string(),
        }))
    }
}

/// Compilation options for SQL backend of the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target and dialect to compile to.
    pub target: Target,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,

    /// Whether to use ANSI colors in error messages.
    pub color: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: true,
            target: Target::Sql(None),
            signature_comment: true,
            color: false,
        }
    }
}

impl Options {
    pub fn no_format(mut self) -> Self {
        self.format = false;
        self
    }

    pub fn with_signature_comment(mut self, signature_comment: bool) -> Self {
        self.signature_comment = signature_comment;
        self
    }

    pub fn no_signature(mut self) -> Self {
        self.signature_comment = false;
        self
    }

    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }

    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

/// Parse PRQL into a PL AST
pub fn prql_to_pl(prql: &str) -> Result<Vec<ast::pl::Stmt>, ErrorMessages> {
    let sources = SourceTree::from(prql);

    parser::parse(&sources)
        .map(|x| x.sources.into_values().next().unwrap())
        .map_err(error::downcast)
        .map_err(|e| e.composed(&prql.into(), false))
}

/// Parse PRQL into a PL AST
pub fn prql_to_pl_tree(prql: &SourceTree) -> Result<SourceTree<Vec<ast::pl::Stmt>>, ErrorMessages> {
    parser::parse(prql)
        .map_err(error::downcast)
        .map_err(|e| e.composed(prql, false))
}

/// Perform semantic analysis and convert PL to RQ.
pub fn pl_to_rq(pl: Vec<ast::pl::Stmt>) -> Result<ast::rq::Query, ErrorMessages> {
    let source_tree = SourceTree::single(PathBuf::new(), pl);
    semantic::resolve_and_lower(source_tree, &[]).map_err(error::downcast)
}

/// Perform semantic analysis and convert PL to RQ.
pub fn pl_to_rq_tree(
    pl: SourceTree<Vec<ast::pl::Stmt>>,
    main_path: &[String],
) -> Result<ast::rq::Query, ErrorMessages> {
    semantic::resolve_and_lower(pl, main_path).map_err(error::downcast)
}

/// Generate SQL from RQ.
pub fn rq_to_sql(rq: ast::rq::Query, options: &Options) -> Result<String, ErrorMessages> {
    sql::compile(rq, options).map_err(error::downcast)
}

/// Generate PRQL code from PL AST
pub fn pl_to_prql(pl: Vec<ast::pl::Stmt>) -> Result<String, ErrorMessages> {
    Ok(codegen::write(&pl))
}

/// JSON serialization and deserialization functions
pub mod json {
    use super::*;

    /// JSON serialization
    pub fn from_pl(pl: Vec<ast::pl::Stmt>) -> Result<String, ErrorMessages> {
        serde_json::to_string(&pl).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON deserialization
    pub fn to_pl(json: &str) -> Result<Vec<ast::pl::Stmt>, ErrorMessages> {
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON serialization
    pub fn from_rq(rq: ast::rq::Query) -> Result<String, ErrorMessages> {
        serde_json::to_string(&rq).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON deserialization
    pub fn to_rq(json: &str) -> Result<ast::rq::Query, ErrorMessages> {
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!(e).into())
    }
}

/// All paths are relative to the project root.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceTree<T: Sized + Serialize = String> {
    /// Mapping from file ids into their contents.
    pub sources: HashMap<PathBuf, T>,

    /// Index of source ids to paths. Used to keep [error::Span] lean.
    source_ids: HashMap<usize, PathBuf>,
}

impl<T: Sized + Serialize> SourceTree<T> {
    pub fn single(path: PathBuf, content: T) -> Self {
        SourceTree {
            sources: [(path.clone(), content)].into(),
            source_ids: [(1, path)].into(),
        }
    }

    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (PathBuf, T)>,
    {
        let mut id_gen = IdGenerator::new();
        let mut res = SourceTree {
            sources: HashMap::new(),
            source_ids: HashMap::new(),
        };

        for (path, content) in iter {
            res.sources.insert(path.clone(), content);
            res.source_ids.insert(id_gen.gen(), path);
        }
        res
    }

    pub fn insert(&mut self, path: PathBuf, content: T) {
        let last_id = self.source_ids.keys().max().cloned().unwrap_or(0);
        self.sources.insert(path.clone(), content);
        self.source_ids.insert(last_id + 1, path);
    }
}

impl<S: ToString> From<S> for SourceTree {
    fn from(source: S) -> Self {
        SourceTree::single(std::path::Path::new("").to_path_buf(), source.to_string())
    }
}

#[cfg(test)]
mod tests_lib {
    use crate::Target;
    use insta::assert_debug_snapshot;
    use std::str::FromStr;

    #[test]
    fn test_target_from_str() {
        assert_debug_snapshot!(Target::from_str("sql.postgres"), @r###"
        Ok(
            Sql(
                Some(
                    Postgres,
                ),
            ),
        )
        "###);

        assert_debug_snapshot!(Target::from_str("sql.poostgres"), @r###"
        Err(
            Error {
                kind: Error,
                span: None,
                reason: NotFound {
                    name: "\"sql.poostgres\"",
                    namespace: "target",
                },
                hints: [],
                code: None,
            },
        )
        "###);

        assert_debug_snapshot!(Target::from_str("postgres"), @r###"
        Err(
            Error {
                kind: Error,
                span: None,
                reason: NotFound {
                    name: "\"postgres\"",
                    namespace: "target",
                },
                hints: [],
                code: None,
            },
        )
        "###);
    }
    #[test]
    fn test_target_names() {
        for name in Target::names() {
            assert_debug_snapshot!(name, Target::from_str(&name));
        }
    }
}
