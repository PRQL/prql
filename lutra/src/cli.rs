use clap::Parser;
use lutra::{Action, Command};

fn main() {
    env_logger::builder().format_timestamp(None).init();

    let action = Command::parse();

    let res = match action.command {
        Action::Print(cmd) => lutra::run(cmd),
        Action::Discover(cmd) => lutra::discover(cmd),
    };

    match res {
        Ok(_) => {}
        Err(err) => {
            let errors = prql_compiler::downcast(err);

            println!("{errors}");
            std::process::exit(1);
        }
    }
}
