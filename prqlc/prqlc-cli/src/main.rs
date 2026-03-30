#[cfg(not(target_family = "wasm"))]
mod cli;

#[cfg(not(target_family = "wasm"))]
fn main() -> color_eyre::eyre::Result<()> {
    // Use a larger stack size (8 MiB) to avoid stack overflows on Windows,
    // where the default stack is only 1 MiB.
    const STACK_SIZE: usize = 8 * 1024 * 1024;

    let thread = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(cli::main)
        .expect("failed to spawn main thread");

    thread.join().expect("main thread panicked")?;
    Ok(())
}

#[cfg(target_family = "wasm")]
fn main() {
    panic!("prqlc-cli cannot be built for wasm targets.");
}
