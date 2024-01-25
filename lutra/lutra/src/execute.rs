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

/*
OBSOLETE
/// Determine the path of the output parquet file.
fn compose_output_path(pipeline_ident: &Ident, project: &ProjectTree) -> Result<String> {
    let module_ident = pipeline_ident
        .clone()
        .pop()
        .unwrap_or_else(|| Ident::from_name("_project"));
    let mut output_path = project.path.clone();
    for segment in module_ident.path {
        output_path.push(segment)
    }
    output_path.push(format!("{}.main.parquet", module_ident.name));
    let output_path = (output_path.into_os_string().into_string())
        .map_err(|s| anyhow::anyhow!("invalid path {s:?}"))?;
    Ok(output_path)
}
*/
