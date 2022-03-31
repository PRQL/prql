mod context;
mod materializer;
mod reporting;
mod resolver;

pub use self::context::{Context, Declaration, VarDec};
pub use materializer::{materialize, SelectedColumns};
pub use reporting::print;
pub use resolver::resolve;

use crate::ast::{Item, Node, Pipeline};
use crate::utils::IntoOnly;
use anyhow::Result;

/// Resolve all variable and function calls then materialize them into their declared values.
///
/// Can work with previously resolved context (defined functions, variables).
/// Also returns materialized columns that can be converted into items for SELECT
pub fn process(
    nodes: Vec<Node>,
    context: Option<Context>,
) -> Result<(Vec<Node>, Context, SelectedColumns)> {
    let (nodes, context) = resolve(nodes, context)?;
    materialize(nodes, context)
}

/// Utility wrapper. See [process]
pub fn process_pipeline(
    pipeline: Pipeline,
    context: Option<Context>,
) -> Result<(Pipeline, Context, SelectedColumns)> {
    let (nodes, context, select) = process(vec![Item::Pipeline(pipeline).into()], context)?;
    let pipeline = nodes.into_only()?.item.into_pipeline()?;
    Ok((pipeline, context, select))
}
