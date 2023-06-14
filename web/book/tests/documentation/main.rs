/// As well as the examples in the book, we also test the examples in the
/// website & README in this integration test binary.
mod book;
mod readme;
mod website;

use ::prql_compiler::Options;

fn compile(prql: &str) -> Result<String, prql_compiler::ErrorMessages> {
    anstream::ColorChoice::Never.write_global();
    prql_compiler::compile(prql, &Options::default().no_signature())
}
