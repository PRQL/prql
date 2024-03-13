use std::path::Path;

use anyhow::Result;

use crate::project::DatabaseModule;

pub fn open(db: &DatabaseModule, project_root: &Path) -> Result<rusqlite::Connection> {
    // convert relative to absolute path
    let mut sqlite_file_abs = project_root.to_path_buf();
    sqlite_file_abs.push(&db.connection_params.file_relative);
    let sqlite_file_abs = sqlite_file_abs.as_os_str().to_str().unwrap();

    // init SQLite
    let sqlite_conn = rusqlite::Connection::open(sqlite_file_abs)?;

    Ok(sqlite_conn)
}
