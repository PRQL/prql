mod compile;
mod discover;
mod execute;
mod project;

use std::path::PathBuf;

use anyhow::Result;

#[cfg(feature = "clap")]
use clap::{Parser, Subcommand};

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct Command {
    #[cfg_attr(feature = "clap", clap(subcommand))]
    pub command: Action,
}

#[cfg_attr(feature = "clap", derive(Subcommand))]
pub enum Action {
    /// Read the project
    Discover(DiscoverParams),

    /// Discover, compile, execute and print
    Print(PrintParams),
}

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct DiscoverParams {
    #[cfg_attr(feature = "clap", arg(default_value = "."))]
    project_path: PathBuf,
}

pub fn discover(params: DiscoverParams) -> Result<()> {
    let project = discover::read_project(params.project_path)?;

    println!("{project}");
    Ok(())
}

#[cfg_attr(feature = "clap", derive(Parser))]
pub struct PrintParams {
    #[cfg_attr(feature = "clap", command(flatten))]
    discover: DiscoverParams,
}

pub fn run(params: PrintParams) -> Result<()> {
    let mut project = discover::read_project(params.discover.project_path)?;

    let database_module = compile::compile(&mut project)?;

    for pipeline_ident in project.pipelines.keys() {
        execute::execute(&project, &database_module, pipeline_ident)?;
    }

    Ok(())
}
