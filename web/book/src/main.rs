// We don't need to run this with wasm, and the features that `mdbook` uses of
// `clap`'s don't support wasm.
#[cfg(not(target_family = "wasm"))]
fn main() {
    use mdbook_preprocessor_boilerplate::run;
    use mdbook_prql::ComparisonPreprocessor;
    run(
        ComparisonPreprocessor,
        "Create comparison examples between PRQL & SQL",
    );
}

#[cfg(target_family = "wasm")]
fn main() -> ! {
    panic!("Not used as a binary in wasm (but it seems cargo insists we have a `main` function).")
}
