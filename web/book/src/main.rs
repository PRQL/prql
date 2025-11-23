// We don't need to run this with wasm, and the features that `mdbook` uses of
// `clap`'s don't support wasm.
#[cfg(not(target_family = "wasm"))]
fn main() {
    use mdbook_preprocessor::{parse_input, Preprocessor};
    use mdbook_prql::ComparisonPreprocessor;
    use std::io;
    use std::process;

    let preprocessor = ComparisonPreprocessor;

    // Handle the supports subcommand
    if let Some(arg) = std::env::args().nth(1) {
        if arg == "supports" {
            let renderer = std::env::args().nth(2).unwrap_or_else(|| {
                eprintln!("mdbook-prql: missing renderer argument for 'supports' subcommand");
                process::exit(1);
            });

            let supports = preprocessor
                .supports_renderer(&renderer)
                .expect("supports_renderer should not fail");
            process::exit(if supports { 0 } else { 1 });
        }
    }

    // Parse input from stdin
    let (ctx, book) = parse_input(io::stdin()).unwrap_or_else(|e| {
        eprintln!("mdbook-prql: failed to parse JSON input from mdbook: {}", e);
        process::exit(1);
    });

    // Run the preprocessor
    let processed_book = preprocessor.run(&ctx, book).unwrap_or_else(|e| {
        eprintln!("mdbook-prql: failed to process PRQL code blocks: {}", e);
        process::exit(1);
    });

    // Serialize and output result
    let output = serde_json::to_string(&processed_book).unwrap_or_else(|e| {
        eprintln!("mdbook-prql: failed to serialize book to JSON: {}", e);
        process::exit(1);
    });
    println!("{}", output);
}

#[cfg(target_family = "wasm")]
fn main() -> ! {
    panic!("Not used as a binary in wasm (but it seems cargo insists we have a `main` function).")
}
