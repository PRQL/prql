#[cfg(all(not(target_family = "wasm"), feature = "cli"))]
mod cli;

#[cfg(all(not(target_family = "wasm"), feature = "cli"))]
fn main() -> color_eyre::eyre::Result<()> {
    cli::main()
}

#[cfg(any(target_family = "wasm", not(feature = "cli")))]
fn main() {
    panic!("Crate is not built with the `cli` feature enabled, or was built for a wasm target.");
}
