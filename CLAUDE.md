# Claude

## Development Workflow

Use a tight inner loop for fast feedback, comprehensive outer loop before
returning to user:

**Inner loop** (fast, focused, <5s):

```sh
# Run fast tests on core packages (from project root)
task prqlc:test

# Unit tests filtered by test name
cargo insta test -p prqlc --lib -- resolver

# Integration tests filtered by test name
cargo insta test -p prqlc --test integration -- date
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

### Test Strategy

**Prefer small inline `insta` snapshot tests** over full integration tests:

- **Use inline tests** for most bug fixes and small features
  - Add `#[test]` functions in a `#[cfg(test)]` module at the end of the file
  - Use `insta::assert_snapshot!` for compact, readable test assertions
  - Fast to run, easy to review in PRs

- **Use integration tests** (`prqlc/tests/integration/queries/*.prql`) only
  when:
  - Developing large, complex features that need comprehensive testing
  - Testing end-to-end behavior across multiple compilation stages
  - The test requires external resources or multi-file scenarios

Example of a good inline test:

```rust
#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    #[test]
    fn test_my_feature() {
        let query = "from employees | filter country == 'USA'";
        assert_snapshot!(crate::tests::compile(query).unwrap(), @"");
    }
}
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
task lint
```

## Error Messages

Error messages should avoid 2nd person (you/your). Use softer modal verbs like
"might" for a friendlier tone:

- ❌ "are you missing `from` statement?" → ✅ "`from` statement might be
  missing?"
- ❌ "did you forget to specify the column name?" → ✅ "column name might be
  missing?"
- ❌ "you can only use X" → ✅ "X requires Y" (for hard constraints)
- ❌ "Have you forgotten an argument?" → ✅ "Argument might be missing?"

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

## Releases & Environment

For releases or environment issues, see
`web/book/src/project/contributing/development.md`.
