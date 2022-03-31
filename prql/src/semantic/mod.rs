mod materializer;
mod reporting;
mod resolver;
mod scope;

pub use self::scope::{Context, Declaration, VarDec};
pub use materializer::{materialize, SelectedColumns};
pub use reporting::print;
pub use resolver::resolve;

use crate::ast::{Item, Node, Pipeline};
use crate::utils::IntoOnly;
use anyhow::Result;

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
