#[cfg(not(target_family = "wasm"))]
fn main() {
    use clap::Parser;
    use lutra::{Action, Command};

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

#[cfg(target_family = "wasm")]
fn main() {
    panic!("Crate was built for a wasm target.");
}
