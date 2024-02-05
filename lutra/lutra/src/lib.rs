// We could be a bit more selective if we wanted this to work with wasm, but at
// the moment too many of the dependencies aren't compatible.
#![cfg(not(target_family = "wasm"))]

mod compile;
mod discover;
mod execute;
mod project;

use std::path::PathBuf;

use anyhow::Result;

#[cfg(feature = "clap")]
use clap::{Parser, Subcommand};
use project::ProjectTree;
use prqlc::ir::pl::Ident;

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct Command {
    #[cfg_attr(feature = "clap", clap(subcommand))]
    pub command: Action,
}

#[cfg_attr(feature = "clap", derive(Subcommand))]
pub enum Action {
    /// Read the project
    Discover(DiscoverParams),

    /// Discover, compile, execute
    Execute(ExecuteParams),
}

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct DiscoverParams {
    /// Path to the project directory
    #[cfg_attr(feature = "clap", arg(default_value = "."))]
    pub project_path: PathBuf,
}

pub fn discover(params: DiscoverParams) -> Result<ProjectTree> {
    discover::read_project(params.project_path)
}

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct ExecuteParams {
    #[cfg_attr(feature = "clap", command(flatten))]
    pub discover: DiscoverParams,

    /// Only execute the expression with this path.
    pub expression_path: Option<String>,
}

pub fn execute(params: ExecuteParams) -> Result<Vec<(Ident, execute::Relation)>> {
    let mut project = discover::read_project(params.discover.project_path)?;

    let database_module = compile::compile(&mut project)?;

    let mut res = Vec::new();
    if let Some(expression_path) = params.expression_path {
        // specified expression

        let expr_ident = Ident::from_path(expression_path.split('.').collect());

        let rel = execute::execute(&project, &database_module, &expr_ident)?;
        res.push((expr_ident.clone(), rel));
    } else {
        // all expressions

        for expr_ident in project.exprs.keys() {
            let rel = execute::execute(&project, &database_module, expr_ident)?;

            res.push((expr_ident.clone(), rel));
        }
    }

    Ok(res)
}
