# Claude

## Tests

Prefer `cargo insta` tests.

When running tests, prefer:

```sh
# Run tests and automatically accept snapshot changes
cargo insta test --accept

# Run tests in a specific package
cargo insta test -p prqlc-parser --accept

# Run tests matching a specific pattern
cargo insta test -p prqlc --test integration -- date
```

Prefer inline snapshots for almost all tests:

```rust
insta::assert_snapshot!(result, @"expected output");
```

Initializing the test with:

```rust
insta::assert_snapshot!(result, @"");
```

...and then running the test commands above with `--accept` will then fill in
the result.

To run all tests, accepting snapshots, run

```sh
task test-all
```

## Running the CLI

For viewing `prqlc` output, for any stage of the compilation process:

```sh
# Compile PRQL to SQL
cargo run -p prqlc -- compile "from employees | filter country == 'USA'"

# Format PRQL code
cargo run -p prqlc -- fmt "from employees | filter country == 'USA'"

# See all available commands
cargo run -p prqlc -- --help
```

## Linting

Run all lints with

```sh
task test-lint
```

## Documentation

For Claude to view crate documentation:

```sh
# Build documentation for a specific crate
cargo doc -p prqlc

# View the generated HTML documentation with the View tool
# The docs are generated at target/doc/{crate_name}/index.html
View target/doc/prqlc/index.html

# For specific module documentation
View target/doc/prqlc/module_name/index.html

# For function documentation
View target/doc/prqlc/fn.compile.html
```
