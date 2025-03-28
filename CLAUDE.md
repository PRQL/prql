# Claude

## Tests

Prefer `cargo insta` tests.

When running tests, prefer:

```bash
# Run tests and automatically accept snapshot changes
cargo insta test --accept

# Run tests in a specific package
cargo insta test -p prqlc-parser --accept

# Run tests matching a specific pattern
cargo insta test -p prqlc --test integration -- date
```

Prefer inline snapshots for small outputs:

```rust
insta::assert_snapshot!(result, @"expected output");
```

## Running the CLI

For viewing `prqlc` output, for any stage of the compilation process:

```bash
# Compile PRQL to SQL
cargo run -p prqlc -- compile "from employees | filter country == 'USA'"

# Format PRQL code
cargo run -p prqlc -- fmt "from employees | filter country == 'USA'"

# See all available commands
cargo run -p prqlc -- --help
```

## Documentation

For Claude to view crate documentation:

```bash
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
