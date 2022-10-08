mod complexity;
mod context;
mod declarations;
mod frame;
mod name_resolver;
mod reporting;
mod scope;
mod transforms;

use crate::ast::{Node, Query};

pub use self::context::Context;
pub use self::declarations::{Declaration, Declarations};
pub use self::frame::{Frame, FrameColumn};
pub use self::scope::{split_var_name, Scope};
pub use reporting::{collect_frames, label_references};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(query: Query, context: Option<Context>) -> anyhow::Result<(Vec<Node>, Context)> {
    let context = context.unwrap_or_else(load_std_lib);

    let (nodes, context) = name_resolver::resolve_names(query, context)?;
    Ok((nodes, context))
}

pub fn load_std_lib() -> Context {
    use crate::parse;
    let std_lib = include_str!("./stdlib.prql");
    let nodes = parse(std_lib).unwrap().nodes;

    let (_, context) = name_resolver::resolve_nodes(nodes, Context::default()).unwrap();
    context
}
