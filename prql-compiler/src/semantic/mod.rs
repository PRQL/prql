mod complexity;
mod context;
mod declarations;
mod frame;
mod name_resolver;
mod reporting;
mod scope;
mod transforms;
mod type_resolver;

use crate::ast::Node;

pub use self::context::Context;
pub use self::declarations::{Declaration, Declarations};
pub use self::frame::{Frame, FrameColumn};
pub use self::scope::{split_var_name, Scope};
pub use name_resolver::resolve_names;
pub use type_resolver::resolve_types;
pub use reporting::{collect_frames, label_references};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(nodes: Vec<Node>, context: Option<Context>) -> anyhow::Result<(Vec<Node>, Context)> {
  resolve_names(nodes, context).and_then(|(n, c)| resolve_types(n, c))
}
