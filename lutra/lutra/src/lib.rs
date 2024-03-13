//! Query runner for PRQL
//!
//! Takes a project tree of source files, compiles PRQL and executes the queries in databases.
//! Defines database connection parameters in .prql files using `@lutra` annotations.
//!
//! Works in following stages:
//! - discover: walk over a directory in the file system to find .prql source files,
//! - compile: use prqlc to compile PRQL to SQL and then find @lutra annotations,
//! - execute: connect to databases to execute the queries and return results as Apache Arrow record batches.
//!
//! For executing the queries and converting to Apache Arrow, lutra uses
//! [connector_arrow](https://docs.rs/connector_arrow/latest/) crate.

// We could be a bit more selective if we wanted this to work with wasm, but at
// the moment too many of the dependencies aren't compatible.
#![cfg(not(target_family = "wasm"))]

mod compile;
mod connection;
mod discover;
pub mod editing;
mod execute;
mod project;
mod pull_schema;

pub use compile::{compile, CompileParams};
pub use discover::{discover, DiscoverParams};
pub use execute::{execute, ExecuteParams};
pub use project::{ProjectCompiled, ProjectDiscovered};
pub use pull_schema::{pull_schema, PullSchemaParams};
