use clap::Parser;
use color_eyre::eyre::Result;
use prql::Cli;
use std::process::exit;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut cli = Cli::parse();

    if let Err(error) = cli.execute() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}
