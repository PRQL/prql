use std::process::exit;

#[cfg(feature = "cli")]
fn main() -> color_eyre::eyre::Result<()> {
    use clap::Parser;
    use prql_compiler::Cli;

    color_eyre::install()?;
    let mut cli = Cli::parse();

    if let Err(error) = cli.run() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}

#[cfg(not(feature = "cli"))]
fn main() -> ! {
    panic!("Not used as a binary in wasm (but it seems cargo insists we have a `main` function).")
}
