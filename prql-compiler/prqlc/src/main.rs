// We put all the code apart from a facade in `lib.rs` so we can easily disable
// its compliation for wasm targets.
//
// We still want to allow compilation for wasm, because we compile the whole
// workspace for wasm in our tests.

#[cfg(not(target_family = "wasm"))]
fn main() -> color_eyre::eyre::Result<()> {
    prqlc::main()
}

#[cfg(target_family = "wasm")]
fn main() {
    panic!("WASM not supported by `prqlc`. `prql-compiler` is the library.");
}
