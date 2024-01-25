use anyhow::Result;
use connectorx::prelude::*;
use prql_compiler::ir::pl::Ident;

use crate::{compile::DatabaseModule, project::ProjectTree};

pub fn execute(project: &ProjectTree, db: &DatabaseModule, pipeline_ident: &Ident) -> Result<()> {
    log::info!("executing {pipeline_ident}");

    // convert relative to absolute path
    let mut sqlite_file_abs = project.path.clone();
    sqlite_file_abs.push(&db.connection_params.file_relative);
    let sqlite_file_abs = sqlite_file_abs.as_os_str().to_str().unwrap();

    // init SQLite
    let source = SQLiteSource::new(sqlite_file_abs, 10)?;

    let pipeline = &project.pipelines[pipeline_ident];
    log::debug!("executing sql: {pipeline}");

    let mut destination = Arrow2Destination::new();
    let dispatcher = Dispatcher::<SQLiteSource, Arrow2Destination, SQLiteArrow2Transport>::new(
        source,
        &mut destination,
        &[pipeline.as_str()],
        None,
    );

    let res = dispatcher.run();
    if let Err(err) = res {
        println!("{err:?}");
        return Err(err.into());
    }

    let arrow = destination.polars()?;
    println!("{pipeline_ident}:\n{arrow}");

    Ok(())
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
