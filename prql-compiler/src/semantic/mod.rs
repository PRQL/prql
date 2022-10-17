mod context;
mod declarations;
mod lowering;
mod reporting;
mod resolver;
mod scope;
mod transforms;
mod type_resolver;

pub use self::context::Context;
pub use self::declarations::{Declaration, Declarations};
pub use self::scope::{split_var_name, Scope};
pub use reporting::{collect_frames, label_references};

use crate::ast::frame::{Frame, FrameColumn};
use crate::ast::Stmt;
use crate::ir::Query;

use anyhow::Result;

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(statements: Vec<Stmt>, context: Option<Context>) -> Result<(Query, Context)> {
    let context = context.unwrap_or_else(load_std_lib);

    let (statements, context) = resolver::resolve(statements, context)?;

    // TODO: make resolve return only query and remove this clone here:
    let query = lowering::lower_ast_to_ir(statements, context.clone())?;

    Ok((query, context))
}

pub fn load_std_lib() -> Context {
    use crate::parse;
    let std_lib = include_str!("./stdlib.prql");
    let statements = parse(std_lib).unwrap();

    let (_, context) = resolver::resolve(statements, Context::default()).unwrap();
    context
}
