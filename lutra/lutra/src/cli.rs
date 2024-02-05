#[cfg(target_family = "wasm")]
fn main() {
    panic!("Crate was built for a wasm target.");
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    use clap::Parser;
    use lutra::{Action, Command};

    env_logger::builder().format_timestamp(None).init();

    let action = Command::parse();

    let res = match action.command {
        Action::Execute(cmd) => printing::execute_and_print(cmd),
        Action::Discover(cmd) => printing::discover_and_print(cmd),
    };

    match res {
        Ok(_) => {}
        Err(err) => {
            let errors = prqlc::downcast(err);

            println!("{errors}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
mod printing {
    pub fn execute_and_print(cmd: lutra::ExecuteParams) -> anyhow::Result<()> {
        let relations = lutra::execute(cmd)?;

        for (ident, relation) in relations {
            let rel_display = arrow::util::pretty::pretty_format_batches(&relation)?;

            println!("{ident}:\n{rel_display}");
        }
        Ok(())
    }

    pub fn discover_and_print(cmd: lutra::DiscoverParams) -> anyhow::Result<()> {
        let project = lutra::discover(cmd)?;

        println!("{project}");
        Ok(())
    }
}
