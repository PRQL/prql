// All copied from `mdbook_preprocessor_boilerplate` apart from the function
// which does the replacement.
// This file is licensed under GPL-3.0 then. We don't link against it from PRQL.

// We don't need to run this with wasm, and the features that `mdbook` uses of
// `clap`'s don't support wasm.
#[cfg(not(target_family = "wasm"))]
fn main() {
    use mdbook_prql::{run, ComparisonPreprocessor};
    eprintln!("Running comparison preprocessor");
    run(
        ComparisonPreprocessor,
        "comparison-preprocessor",
        "Create comparison examples between PRQL & SQL",
    );
}

#[cfg(target_family = "wasm")]
fn main() -> ! {
    panic!("Not used as a binary in wasm (but it seems cargo insists we have a `main` function).")
}
