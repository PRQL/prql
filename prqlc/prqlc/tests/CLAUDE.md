# Tests

## Structure

- **`integration/sql.rs`** — Unit tests for PRQL→SQL compilation. Fast, focused,
  preferred for most changes.
- **`integration/queries/*.prql`** — Integration tests generating 6 snapshots
  each (different SQL dialects). Use sparingly.
- **`integration/error_messages.rs`** — Tests for compiler error diagnostics.
- **`integration/dbs/`** — Database runners for executing queries against real
  databases.

## Guidelines

**80% unit tests, 20% integration tests.** Integration tests in
`integration/queries/` are verbose — each `.prql` file generates six snapshot
files. Prefer unit tests in [`integration/sql.rs`](./integration/sql.rs):

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
