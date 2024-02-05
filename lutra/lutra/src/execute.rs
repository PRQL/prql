use anyhow::Result;
use prqlc::{ir::pl::Ident, Error};

use crate::{compile::DatabaseModule, project::ProjectTree};

pub type Relation = Vec<arrow::record_batch::RecordBatch>;

pub fn execute(
    project: &ProjectTree,
    db: &DatabaseModule,
    pipeline_ident: &Ident,
) -> Result<Relation> {
    log::info!("executing {pipeline_ident}");

    // convert relative to absolute path
    let mut sqlite_file_abs = project.path.clone();
    sqlite_file_abs.push(&db.connection_params.file_relative);
    let sqlite_file_abs = sqlite_file_abs.as_os_str().to_str().unwrap();

    // init SQLite
    let mut sqlite_conn = rusqlite::Connection::open(sqlite_file_abs)?;

    let Some(pipeline) = project.exprs.get(pipeline_ident) else {
        return Err(
            Error::new_simple(format!("cannot find expression: `{pipeline_ident}`")).into(),
        );
    };
    log::debug!("executing sql: {pipeline}");

    let batches = connector_arrow::query_one(&mut sqlite_conn, pipeline)?;

    Ok(batches)
}
