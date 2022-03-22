// TODO: We can't currently build without cli feature as we then don't have a
// `main` functions — we need to work through building as a library, so we can
// use with wasm; ref GH #175.
#[cfg(feature = "cli")]
fn main() -> color_eyre::eyre::Result<()> {
    use clap::Parser;
    use prql::Cli;
    use std::process::exit;

    color_eyre::install()?;
    let mut cli = Cli::parse();

    if let Err(error) = cli.execute() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}
