# Claude

## Development Workflow

Use a tight inner loop for fast feedback, comprehensive outer loop before
returning to user:

**Inner loop** (fast, focused, <5s):

```sh
# Run lints on changed files
task test-lint

# Run specific tests you're working on
cargo insta test -p prqlc --test integration -- date

# Run unit tests for a specific module
cargo insta test -p prqlc --lib semantic::resolver
```

**Outer loop** (comprehensive, ~1min, before returning to user):

```sh
# Run everything - this is required before returning
task test-all
```

The test suite is configured to minimize token usage:

- **Nextest** only shows failures and slow tests (not 600 PASS lines)
- **Cargo builds** use `--quiet` flag (no compilation spam)
- **Result**: ~52% reduction in output (1128 → 540 lines, ~4.5k tokens)

## Tests

Prefer inline snapshots for almost all tests:

```rust
insta::assert_snapshot!(result, @"expected output");
```

Initialize tests with empty snapshots, then run with `--accept`:

```rust
insta::assert_snapshot!(result, @"");
```

The test commands above with `--accept` will fill in the result automatically.

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
