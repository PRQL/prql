# Tests

## Structure

- **`integration/sql.rs`** — Unit tests for PRQL→SQL compilation. Fast, focused,
  preferred for most changes.
- **`integration/queries/*.prql`** — Integration tests generating up to 6
  snapshots each (`lex`, `compile`, `compileall`, `fmt`, `debug_lineage`,
  `results`), run by `integration/queries.rs`. Use sparingly. Header comments
  such as `# sqlite:skip` or `# mssql:test` select which dialects run the query
  against a database; `# generic:skip` also drops the `compile` and
  `compileall` snapshots.
- **`integration/error_messages.rs`** — Tests for compiler error diagnostics.
- **`integration/bad_error_messages.rs`** — Error messages known to be poor.
  When a fix improves one, move its test to `error_messages.rs`.
- **`integration/dbs/`** — Database runners for executing queries against real
  databases.
- **`integration/project/`** — A multi-file project used by the CLI tests in
  `src/cli/test.rs`.

## Guidelines

**80% unit tests, 20% integration tests.** Integration tests in
`integration/queries/` are verbose — each `.prql` file generates up to six
snapshot files. Prefer unit tests in
[`integration/sql.rs`](./integration/sql.rs):

```rust
#[test]
fn test_my_feature() {
    assert_snapshot!(compile(r#"
    from foo
    select bar
    "#).unwrap(), @"...");
}
```

New integration test files are appropriate when validating against real
databases or testing behavior across compilation stages. Extend existing tests
when possible.
