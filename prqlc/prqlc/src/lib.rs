//! # prqlc
//!
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
#![doc = include_str!("../../ARCHITECTURE.md")]
//!
//! ## Common use-cases
//!
//! - Compile PRQL queries to SQL at run time.
//!
//!   ```
//!   # fn main() -> Result<(), prqlc::ErrorMessages> {
//!   let sql = prqlc::compile(
//!       "from albums | select {title, artist_id}",
//!        &prqlc::Options::default().no_format()
//!   )?;
//!   assert_eq!(&sql[..35], "SELECT title, artist_id FROM albums");
//!   # Ok(())
//!   # }
//!   ```
//!
//! - Compile PRQL queries to SQL at build time.
//!
//!   For inline strings, use the `prqlc-macros` crate; for example:
//!   ```ignore
//!   let sql: &str = prql_to_sql!("from albums | select {title, artist_id}");
//!   ```
//!
//!   For compiling whole files (`.prql` to `.sql`), call `prqlc` from
//!   `build.rs`. See [this example
//!   project](https://github.com/PRQL/prql/tree/main/prqlc/prqlc/examples/compile-files).
//!
//! - Compile, format & debug PRQL from command line.
//!
//!   ```sh
//!   $ cargo install --locked prqlc
//!   $ prqlc compile query.prql
//!   ```
//!
//! ## Feature flags
//!
//! The following feature flags are available:
//!
//! * `cli`: enables the `prqlc` CLI binary. This is enabled by default. When
//!   consuming this crate from another rust library, it can be disabled.
//! * `test-dbs`: enables the `prqlc` in-process test databases as part of the
//!   crate's tests. This significantly increases compile times so is not
//!   enabled by default.
//! * `test-dbs-external`: enables the `prqlc` external test databases,
//!   requiring a docker container with the test databases to be running. Check
//!   out the [integration tests](https://github.com/PRQL/prql/tree/main/prqlc/prqlc/tests/integration/dbs)
//!   for more details.
//! * `serde_yaml`: Enables serialization and deserialization of ASTs to YAML.
//!
//! ## Large binary sizes
//!
//! For Linux users, the binary size contributed by this crate will probably be
//! quite large (>20MB) by default. That is because it includes a lot of
//! debuginfo symbols from our parser. They can be removed by adding the
//! following to `Cargo.toml`, reducing the contribution to around 7MB:
//! ```toml
//! [profile.release.package.prqlc]
//! strip = "debuginfo"
//! ```

#![forbid(unsafe_code)]
// Our error type is 128 bytes, because it contains 5 strings & an Enum, which
// is exactly the default warning level. Given we're not that performance
// sensitive, it's fine to ignore this at the moment (and not worth having a
// clippy config file for a single setting). We can consider adjusting it as a
// yak-shaving exercise in the future.
#![allow(clippy::result_large_err)]

use std::sync::OnceLock;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use anstream::adapter::strip_str;
use semver::Version;
use serde::{Deserialize, Serialize};
use strum::VariantNames;

pub use error_message::{ErrorMessage, ErrorMessages, SourceLocation};
pub use prqlc_parser::error::{Error, ErrorSource, Errors, MessageKind, Reason, WithErrorInfo};
pub use prqlc_parser::lexer::lr;
pub use prqlc_parser::parser::pr;
pub use prqlc_parser::span::Span;

mod codegen;
pub mod debug;
mod error_message;
pub mod ir;
pub mod parser;
pub mod semantic;
pub mod sql;
#[cfg(feature = "cli")]
pub mod utils;
#[cfg(not(feature = "cli"))]
pub(crate) mod utils;

pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Get the version of the compiler. This is determined by, in order:
/// - An optional environment variable `PRQL_VERSION_OVERRIDE`. Note that this
///   needs to be set the first time this funciton is called, since it's stored
///   in a static.
/// - The version defined by `git describe`
/// - The version in the cargo manifest
pub fn compiler_version() -> &'static Version {
    static COMPILER_VERSION: OnceLock<Version> = OnceLock::new();
    COMPILER_VERSION.get_or_init(|| {
        if let Ok(prql_version_override) = std::env::var("PRQL_VERSION_OVERRIDE") {
            return Version::parse(&prql_version_override).unwrap_or_else(|e| {
                panic!(
                    "Could not parse PRQL version {}\n{}",
                    prql_version_override, e
                )
            });
        }
        let git_version = env!("VERGEN_GIT_DESCRIBE");
        let cargo_version = env!("CARGO_PKG_VERSION");
        Version::parse(git_version).unwrap_or_else(|e| {
            {
                log::info!("Could not parse git version number {}\n{}", git_version, e);
                Version::parse(cargo_version)
            }
            .unwrap_or_else(|e| {
                panic!(
                    "Could not parse prqlc version number {}\n{}",
                    cargo_version, e
                )
            })
        })
    })
}

/// Compile a PRQL string into a SQL string.
///
/// This is a wrapper for:
/// - [prql_to_pl] — Build PL AST from a PRQL string
/// - [pl_to_rq] — Finds variable references, validates functions calls,
///   determines frames and converts PL to RQ.
/// - [rq_to_sql] — Convert RQ AST into an SQL string.
/// # Example
/// Use the prql compiler to convert a PRQL string to SQLite dialect
///
/// ```
/// use prqlc::{compile, Options, Target, sql::Dialect};
///
/// let prql = "from employees | select {name,age}";
/// let opts = Options::default().with_target(Target::Sql(Some(Dialect::SQLite))).with_signature_comment(false).with_format(false);
/// let sql = compile(&prql, &opts).unwrap();
/// println!("PRQL: {}\nSQLite: {}", prql, &sql);
/// assert_eq!("SELECT name, age FROM employees", sql)
///
/// ```
/// See [`sql::Options`](sql/struct.Options.html) and
/// [`sql::Dialect`](sql/enum.Dialect.html) for options and supported SQL
/// dialects.
pub fn compile(prql: &str, options: &Options) -> Result<String, ErrorMessages> {
    let sources = SourceTree::from(prql);

    Ok(&sources)
        .and_then(parser::parse)
        .and_then(|ast| {
            semantic::resolve_and_lower(ast, &[], None)
                .map_err(|e| e.with_source(ErrorSource::NameResolver).into())
        })
        .and_then(|rq| {
            sql::compile(rq, options).map_err(|e| e.with_source(ErrorSource::SQL).into())
        })
        .map_err(|e| {
            let error_messages = ErrorMessages::from(e).composed(&sources);
            match options.display {
                DisplayOptions::AnsiColor => error_messages,
                DisplayOptions::Plain => ErrorMessages {
                    inner: error_messages
                        .inner
                        .into_iter()
                        .map(|e| ErrorMessage {
                            display: e.display.map(|s| strip_str(&s).to_string()),
                            ..e
                        })
                        .collect(),
                },
            }
        })
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

    /// Deprecated: use `display` instead.
    pub color: bool,

    /// Whether to use ANSI colors in error messages. This may be extended to
    /// other formats in the future.
    ///
    /// Note that we don't generally recommend threading a `color` option
    /// through an entire application. Instead, in order of preferences:
    /// - Use a library such as `anstream` to encapsulate presentation logic and
    ///   automatically disable colors when not connected to a TTY.
    /// - Set an environment variable such as `CLI_COLOR=0` to disable any
    ///   colors coming back from this library.
    /// - Strip colors from the output (possibly also with a library such as
    ///   `anstream`).
    pub display: DisplayOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: true,
            target: Target::Sql(None),
            signature_comment: true,
            color: true,
            display: DisplayOptions::AnsiColor,
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

    #[deprecated(note = "`color` is replaced by `display`; see `Options` docs for more details")]
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }

    pub fn with_display(mut self, display: DisplayOptions) -> Self {
        self.display = display;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum DisplayOptions {
    /// Plain text
    Plain,
    /// With ANSI colors
    AnsiColor,
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

/// Lex PRQL source into Lexer Representation.
pub fn prql_to_tokens(prql: &str) -> Result<lr::Tokens, ErrorMessages> {
    prqlc_parser::lexer::lex_source(prql).map_err(|e| {
        e.into_iter()
            .map(|e| e.into())
            .collect::<Vec<ErrorMessage>>()
            .into()
    })
}

/// Parse PRQL into a PL AST
// TODO: rename this to `prql_to_pl_simple`
pub fn prql_to_pl(prql: &str) -> Result<pr::ModuleDef, ErrorMessages> {
    let source_tree = SourceTree::from(prql);
    prql_to_pl_tree(&source_tree)
}

/// Parse PRQL into a PL AST
pub fn prql_to_pl_tree(prql: &SourceTree) -> Result<pr::ModuleDef, ErrorMessages> {
    parser::parse(prql).map_err(|e| ErrorMessages::from(e).composed(prql))
}

/// Perform semantic analysis and convert PL to RQ.
// TODO: rename this to `pl_to_rq_simple`
pub fn pl_to_rq(pl: pr::ModuleDef) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
    semantic::resolve_and_lower(pl, &[], None)
        .map_err(|e| e.with_source(ErrorSource::NameResolver).into())
}

/// Perform semantic analysis and convert PL to RQ.
pub fn pl_to_rq_tree(
    pl: pr::ModuleDef,
    main_path: &[String],
    database_module_path: &[String],
) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
    semantic::resolve_and_lower(pl, main_path, Some(database_module_path))
        .map_err(|e| e.with_source(ErrorSource::NameResolver).into())
}

/// Generate SQL from RQ.
pub fn rq_to_sql(rq: ir::rq::RelationalQuery, options: &Options) -> Result<String, ErrorMessages> {
    sql::compile(rq, options).map_err(|e| e.with_source(ErrorSource::SQL).into())
}

/// Generate PRQL code from PL AST
pub fn pl_to_prql(pl: &pr::ModuleDef) -> Result<String, ErrorMessages> {
    Ok(codegen::WriteSource::write(&pl.stmts, codegen::WriteOpt::default()).unwrap())
}

/// JSON serialization and deserialization functions
pub mod json {
    use super::*;

    /// JSON serialization
    pub fn from_pl(pl: &pr::ModuleDef) -> Result<String, ErrorMessages> {
        serde_json::to_string(pl).map_err(convert_json_err)
    }

    /// JSON deserialization
    pub fn to_pl(json: &str) -> Result<pr::ModuleDef, ErrorMessages> {
        serde_json::from_str(json).map_err(convert_json_err)
    }

    /// JSON serialization
    pub fn from_rq(rq: &ir::rq::RelationalQuery) -> Result<String, ErrorMessages> {
        serde_json::to_string(rq).map_err(convert_json_err)
    }

    /// JSON deserialization
    pub fn to_rq(json: &str) -> Result<ir::rq::RelationalQuery, ErrorMessages> {
        serde_json::from_str(json).map_err(convert_json_err)
    }

    fn convert_json_err(err: serde_json::Error) -> ErrorMessages {
        ErrorMessages::from(Error::new_simple(err.to_string()))
    }
}

/// All paths are relative to the project root.
// We use `SourceTree` to represent both a single file (including a "file" piped
// from stdin), and a collection of files. (Possibly this could be implemented
// as a Trait with a Struct for each type, which would use structure over values
// (i.e. `Option<PathBuf>` below signifies whether it's a project or not). But
// waiting until it's necessary before splitting it out.)
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceTree {
    /// Path to the root of the source tree.
    pub root: Option<PathBuf>,

    /// Mapping from file paths into into their contents.
    /// Paths are relative to the root.
    pub sources: HashMap<PathBuf, String>,

    /// Index of source ids to paths. Used to keep [error::Span] lean.
    source_ids: HashMap<u16, PathBuf>,
}

impl SourceTree {
    pub fn single(path: PathBuf, content: String) -> Self {
        SourceTree {
            sources: [(path.clone(), content)].into(),
            source_ids: [(1, path)].into(),
            root: None,
        }
    }

    pub fn new<I>(iter: I, root: Option<PathBuf>) -> Self
    where
        I: IntoIterator<Item = (PathBuf, String)>,
    {
        let mut res = SourceTree {
            sources: HashMap::new(),
            source_ids: HashMap::new(),
            root,
        };

        for (index, (path, content)) in iter.into_iter().enumerate() {
            res.sources.insert(path.clone(), content);
            res.source_ids.insert((index + 1) as u16, path);
        }
        res
    }

    pub fn insert(&mut self, path: PathBuf, content: String) {
        let last_id = self.source_ids.keys().max().cloned().unwrap_or(0);
        self.sources.insert(path.clone(), content);
        self.source_ids.insert(last_id + 1, path);
    }

    pub fn get_path(&self, source_id: u16) -> Option<&PathBuf> {
        self.source_ids.get(&source_id)
    }
}

impl<S: ToString> From<S> for SourceTree {
    fn from(source: S) -> Self {
        SourceTree::single(PathBuf::from(""), source.to_string())
    }
}

/// Debugging and unstable API functions
pub mod internal {
    use super::*;

    /// Create column-level lineage graph
    pub fn pl_to_lineage(
        pl: pr::ModuleDef,
    ) -> Result<semantic::reporting::FrameCollector, ErrorMessages> {
        let ast = Some(pl.clone());

        let root_module = semantic::resolve(pl).map_err(ErrorMessages::from)?;

        let (main, _) = root_module.find_main_rel(&[]).unwrap();
        let mut fc =
            semantic::reporting::collect_frames(*main.clone().into_relation_var().unwrap());
        fc.ast = ast;

        Ok(fc)
    }

    pub mod json {
        use super::*;

        /// JSON serialization of FrameCollector lineage
        pub fn from_lineage(
            fc: &semantic::reporting::FrameCollector,
        ) -> Result<String, ErrorMessages> {
            serde_json::to_string(fc).map_err(convert_json_err)
        }

        fn convert_json_err(err: serde_json::Error) -> ErrorMessages {
            ErrorMessages::from(Error::new_simple(err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use insta::assert_debug_snapshot;

    use crate::pr::Ident;
    use crate::Target;

    pub fn compile(prql: &str) -> Result<String, super::ErrorMessages> {
        anstream::ColorChoice::Never.write_global();
        super::compile(prql, &super::Options::default().no_signature())
    }

    #[test]
    fn test_starts_with() {
        // Over-testing, from co-pilot, can remove some of them.
        let a = Ident::from_path(vec!["a", "b", "c"]);
        let b = Ident::from_path(vec!["a", "b"]);
        let c = Ident::from_path(vec!["a", "b", "c", "d"]);
        let d = Ident::from_path(vec!["a", "b", "d"]);
        let e = Ident::from_path(vec!["a", "c"]);
        let f = Ident::from_path(vec!["b", "c"]);
        assert!(a.starts_with(&b));
        assert!(a.starts_with(&a));
        assert!(!a.starts_with(&c));
        assert!(!a.starts_with(&d));
        assert!(!a.starts_with(&e));
        assert!(!a.starts_with(&f));
    }

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
