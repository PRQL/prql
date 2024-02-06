use std::collections::HashMap;
use std::path::PathBuf;

use prqlc::ir::pl::Ident;

/// The core object containing PRQL sources, compilation results and lutra annotations.
#[derive(Debug, Default)]
pub struct ProjectDiscovered {
    /// An absolute path to the directory that contains all of the source files.
    pub root_path: PathBuf,

    /// PRQL sources, with the path, relative to the root path.
    pub sources: HashMap<PathBuf, String>,
}

#[derive(Debug)]
pub struct ProjectCompiled {
    pub inner: ProjectDiscovered,

    /// Populated during compilation.
    pub database_module: DatabaseModule,

    /// SQL queries that are ready to be executed in the database, pointed to by `database_module`.
    /// Populated during compilation.
    pub queries: HashMap<Ident, String>,
}

/// A PRQL module that represents a database.
/// It contains variable definitions that represent tables in the database.
/// Connection parameters were extracted from @lutra annotation.
#[derive(Debug)]
pub struct DatabaseModule {
    pub path: Vec<String>,

    // TODO: this should be an enum of all supported databases.
    pub connection_params: SqliteConnectionParams,
}

/// Connection parameters for SQLite.
#[derive(Debug)]
pub struct SqliteConnectionParams {
    pub file_relative: std::path::PathBuf,
}

impl std::fmt::Display for ProjectDiscovered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut r = format!("path: {}\nsources:\n", self.root_path.to_string_lossy());

        for source in self.sources.keys() {
            r += "- ";
            r += &source.to_string_lossy();
            r += "\n";
        }

        f.write_str(&r)
    }
}
