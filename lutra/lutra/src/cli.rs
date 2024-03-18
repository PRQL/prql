#[cfg(target_family = "wasm")]
fn main() {
    panic!("Crate was built for a wasm target.");
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    use clap::Parser;
    use inner::*;

    env_logger::builder().format_timestamp(None).init();

    let action = Command::parse();

    let res = match action.command {
        Action::Discover(cmd) => discover_and_print(cmd),
        Action::Execute(cmd) => execute_and_print(cmd),
        Action::PullSchema(cmd) => pull_schema_and_print(cmd),
    };

    match res {
        Ok(_) => {}
        Err(err) => {
            println!("{err}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
mod inner {
    use clap::{Parser, Subcommand};
    use lutra::{CompileParams, DiscoverParams, ExecuteParams, PullSchemaParams};

    #[derive(Parser)]
    pub struct Command {
        #[clap(subcommand)]
        pub command: Action,
    }

    #[derive(Subcommand)]
    pub enum Action {
        /// Read the project
        Discover(DiscoverCommand),

        /// Discover, compile, execute
        Execute(ExecuteCommand),

        /// Pull schema from data sources
        PullSchema(PullSchemaCommand),
    }

    #[derive(clap::Parser)]
    pub struct DiscoverCommand {
        #[clap(flatten)]
        discover: DiscoverParams,
    }

    pub fn discover_and_print(cmd: DiscoverCommand) -> anyhow::Result<()> {
        let project = lutra::discover(cmd.discover)?;

        println!("{project}");
        Ok(())
    }

    #[derive(clap::Parser)]
    pub struct ExecuteCommand {
        #[clap(flatten)]
        discover: DiscoverParams,

        #[clap(flatten)]
        compile: CompileParams,

        #[clap(flatten)]
        execute: ExecuteParams,
    }

    pub fn execute_and_print(cmd: ExecuteCommand) -> anyhow::Result<()> {
        let project = lutra::discover(cmd.discover)?;

        let project = lutra::compile(project, cmd.compile)?;

        let results = lutra::execute(project, cmd.execute)?;

        for (ident, relation) in results {
            let rel_display = arrow::util::pretty::pretty_format_batches(&relation)?;

            println!("{ident}:\n{rel_display}");
        }
        Ok(())
    }

    #[derive(clap::Parser)]
    pub struct PullSchemaCommand {
        #[clap(flatten)]
        discover: DiscoverParams,

        #[clap(flatten)]
        compile: CompileParams,

        #[clap(flatten)]
        execute: PullSchemaParams,
    }

    pub fn pull_schema_and_print(cmd: PullSchemaCommand) -> anyhow::Result<()> {
        let project = lutra::discover(cmd.discover)?;

        let project = lutra::compile(project, cmd.compile)?;

        let db_mod_decl_id = project.database_module.def_id.unwrap();

        let stmts = lutra::pull_schema(&project, cmd.execute)?;

        use prqlc::ast::*;

        let db_mod_name = project.database_module.path.last().cloned();
        let new_module_def = ModuleDef {
            name: db_mod_name.unwrap_or_default(),
            stmts,
        };

        // TODO: make sure that annotations don't get overwritten

        let new_source = prqlc::pl_to_prql(&new_module_def)?;

        lutra::editing::edit_source_file(&project, db_mod_decl_id, new_source)?;

        Ok(())
    }
}
