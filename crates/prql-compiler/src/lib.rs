//! Compiler for PRQL language. Targets SQL and exposes PL and RQ abstract
//! syntax trees.
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
//! - Compile PRQL queries to SQL at run time.
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
//! - Compile PRQL queries to SQL at build time.
//!
//!     For inline strings, use the `prql-compiler-macros` crate; for example:
//!     ```ignore
//!     let sql: &str = prql_to_sql!("from albums | select {title, artist_id}");
//!     ```
//!
//!     For compiling whole files (`.prql` to `.sql`), call `prql-compiler`
//!     from `build.rs`.
//!     See [this example project](https://github.com/PRQL/prql/tree/main/crates/prql-compiler/examples/compile-files).
//!
//! - Compile, format & debug PRQL from command line.
//!
//!     ```sh
//!     $ cargo install --locked prqlc
//!     $ prqlc compile query.prql
//!     ```
//!
//! ## Feature flags
//!
//! The following [feature flags](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) are available:
//!
//! * `serde_yaml`: adapts the `Serialize` implementation for [`prql_ast::expr::ExprKind::Literal`]
//!   to `serde_yaml`, which doesn't support the serialization of nested enums

#![forbid(unsafe_code)]
// Our error type is 128 bytes, because it contains 5 strings & an Enum, which
// is exactly the default warning level. Given we're not that performance
// sensitive, it's fine to ignore this at the moment (and not worth having a
// clippy config file for a single setting). We can consider adjusting it as a
// yak-shaving exercise in the future.
#![allow(clippy::result_large_err)]

mod codegen;
mod error_message;
pub mod ir;
mod parser;
pub mod semantic;
pub mod sql;
#[cfg(test)]
mod tests;
mod utils;

pub use error_message::{downcast, ErrorMessage, ErrorMessages, SourceLocation, WithErrorInfo};
pub use ir::Span;
pub use prql_ast::error::{Error, Errors, MessageKind, Reason};

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
        .map_err(error_message::downcast)
        .map_err(|e| e.composed(&prql.into()))
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

    /// Whether to use ANSI colors in error messages. This is deprecated and has
    /// no effect.
    ///
    /// Instead, in order of preference:
    /// - Use a library such as `anstream` to encapsulate the presentation
    ///   logic.
    /// - Set an environment variable such as `CLI_COLOR=0` to disable any
    ///   colors coming back from this library.
    /// - Strip colors from the output (possibly also with a library such as `anstream`)
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
    pub fn with_format(mut self, format: bool) -> Self {
        self.format = format;
        self
    }

    pub fn no_format(self) -> Self {
        self.with_format(false)
    }

    pub fn with_signature_comment(mut self, signature_comment: bool) -> Self {
        self.signature_comment = signature_comment;
        self
    }

    pub fn no_signature(self) -> Self {
        self.with_signature_comment(false)
    }

    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }

    #[deprecated(note = "`color` now has no effect; see `Options` docs for more details")]
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

/// Parse PRQL into a PL AST
pub fn prql_to_pl(prql: &str) -> Result<Vec<prql_ast::stmt::Stmt>, ErrorMessages> {
    let sources = SourceTree::from(prql);

    parser::parse(&sources)
        .map(|x| x.sources.into_values().next().unwrap())
        .map_err(error_message::downcast)
        .map_err(|e| e.composed(&prql.into()))
}

/// Parse PRQL into a PL AST
pub fn prql_to_pl_tree(
    prql: &SourceTree,
) -> Result<SourceTree<Vec<prql_ast::stmt::Stmt>>, ErrorMessages> {
    parser::parse(prql)
        .map_err(error_message::downcast)
        .map_err(|e| e.composed(prql))
}

/// Perform semantic analysis and convert PL to RQ.
pub fn pl_to_rq(pl: Vec<prql_ast::stmt::Stmt>) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
    let source_tree = SourceTree::single(PathBuf::new(), pl);
    semantic::resolve_and_lower(source_tree, &[]).map_err(error_message::downcast)
}

/// Perform semantic analysis and convert PL to RQ.
pub fn pl_to_rq_tree(
    pl: SourceTree<Vec<prql_ast::stmt::Stmt>>,
    main_path: &[String],
) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
    semantic::resolve_and_lower(pl, main_path).map_err(error_message::downcast)
}

/// Generate SQL from RQ.
pub fn rq_to_sql(rq: ir::rq::RelationalQuery, options: &Options) -> Result<String, ErrorMessages> {
    sql::compile(rq, options).map_err(error_message::downcast)
}

/// Generate PRQL code from PL AST
pub fn pl_to_prql(pl: Vec<prql_ast::stmt::Stmt>) -> Result<String, ErrorMessages> {
    Ok(codegen::write_stmts(&pl))
}

/// JSON serialization and deserialization functions
pub mod json {
    use super::*;

    /// JSON serialization
    pub fn from_pl(pl: Vec<prql_ast::stmt::Stmt>) -> Result<String, ErrorMessages> {
        serde_json::to_string(&pl).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON deserialization
    pub fn to_pl(json: &str) -> Result<Vec<prql_ast::stmt::Stmt>, ErrorMessages> {
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON serialization
    pub fn from_rq(rq: ir::rq::RelationalQuery) -> Result<String, ErrorMessages> {
        serde_json::to_string(&rq).map_err(|e| anyhow::anyhow!(e).into())
    }

    /// JSON deserialization
    pub fn to_rq(json: &str) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!(e).into())
    }
}

/// All paths are relative to the project root.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceTree<T: Sized + Serialize = String> {
    /// Mapping from file ids into their contents.
    pub sources: HashMap<PathBuf, T>,

    /// Index of source ids to paths. Used to keep [error::Span] lean.
    source_ids: HashMap<u16, PathBuf>,
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
        let mut id_gen = IdGenerator::<usize>::new();
        let mut res = SourceTree {
            sources: HashMap::new(),
            source_ids: HashMap::new(),
        };

        for (path, content) in iter {
            res.sources.insert(path.clone(), content);
            res.source_ids.insert(id_gen.gen() as u16, path);
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

    /// Confirm that all target names can be parsed.
    #[test]
    fn test_target_names() {
        let _: Vec<_> = Target::names()
            .into_iter()
            .map(|name| Target::from_str(&name))
            .collect();
    }
}
