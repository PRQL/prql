#[cfg(all(feature = "cli", not(target_family = "wasm")))]
fn main() -> color_eyre::eyre::Result<()> {
    use clap::Parser;
    use prql_compiler::Cli;
    use std::process::exit;

    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;
    let mut cli = Cli::parse();

    if let Err(error) = cli.run() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}

#[cfg(target_family = "wasm")]
fn main() -> ! {
    panic!("Not used as a binary in wasm (but it seems cargo insists we have a `main` function).")
}

#[cfg(not(feature = "cli"))]
fn main() -> ! {
    panic!("cli feature not enabled")
}
