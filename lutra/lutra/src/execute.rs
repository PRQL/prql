use anyhow::Result;
use prqlc::{ir::pl::Ident, Error};

use super::ProjectCompiled;

pub type Relation = Vec<arrow::record_batch::RecordBatch>;

#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct ExecuteParams {
    /// Only execute the expression with this path.
    pub expression_path: Option<String>,
}

pub fn execute(project: ProjectCompiled, params: ExecuteParams) -> Result<Vec<(Ident, Relation)>> {
    let mut res = Vec::new();
    if let Some(expression_path) = params.expression_path {
        // only the specified expression

        let expr_ident = Ident::from_path(expression_path.split('.').collect());

        let rel = execute_one(&project, &expr_ident)?;
        res.push((expr_ident.clone(), rel));
    } else {
        // all expressions

        for expr_ident in project.queries.keys() {
            let rel = execute_one(&project, expr_ident)?;

            res.push((expr_ident.clone(), rel));
        }
    }

    Ok(res)
}

fn execute_one(project: &ProjectCompiled, pipeline_ident: &Ident) -> Result<Relation> {
    log::info!("executing {pipeline_ident}");
    let db = &project.database_module;
    let project_root = project.inner.root_path.as_path();

    let mut conn = crate::connection::open(db, project_root)?;

    let Some(pipeline) = project.queries.get(pipeline_ident) else {
        return Err(
            Error::new_simple(format!("cannot find expression: `{pipeline_ident}`")).into(),
        );
    };
    log::debug!("executing sql: {pipeline}");

    let batches = connector_arrow::query_one(&mut conn, pipeline)?;

    Ok(batches)
}
