mod complexity;
mod context;
mod declarations;
mod reporting;
mod resolver;
mod scope;
mod transforms;
mod type_resolver;

use crate::ast::frame::{Frame, FrameColumn};
use crate::ast::Query;

pub use self::context::Context;
pub use self::declarations::{Declaration, Declarations};
pub use self::scope::{split_var_name, Scope};
pub use reporting::{collect_frames, label_references};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(query: Query, context: Option<Context>) -> anyhow::Result<(Query, Context)> {
    let context = context.unwrap_or_else(load_std_lib);

    let (query, context) = resolver::resolve(query, context)?;
    Ok((query, context))
}

pub fn load_std_lib() -> Context {
    use crate::parse;
    let std_lib = include_str!("./stdlib.prql");
    let query = parse(std_lib).unwrap();

    let (_, context) = resolver::resolve(query, Context::default()).unwrap();
    context
}
