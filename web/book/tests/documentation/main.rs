#![cfg(not(target_family = "wasm"))]

/// As well as the examples in the book, we also test the examples in the
/// website & README in this integration test binary.
mod book;
mod readme;
mod website;

use ::prqlc::Options;

fn compile(prql: &str) -> Result<String, prqlc::ErrorMessages> {
    prqlc::compile(
        prql,
        &Options::default()
            .no_signature()
            .with_display(prqlc::DisplayOptions::Plain),
    )
}
