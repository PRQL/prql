# PRQL compiler

`prqlc` is the reference implementation of a compiler from PRQL to SQL, written
in Rust.

For more on PRQL, check out the [PRQL website](https://prql-lang.org) or the
[PRQL repo](https://github.com/PRQL/prql).

For more usage examples and the library documentation, check out the
[`prql-compiler` documentation](https://docs.rs/prql-compiler/).

## Installation

```shell
cargo add prql-compiler
```

## Examples

Compile a PRQL string to a SQLite dialect string.

**src/main.rs**

```rust
use prql_compiler::{compile, Options, Target, sql::Dialect};

let prql = "from employees | select {name, age}";
let opts = &Options {
    format: false,
    target: Target::Sql(Some(Dialect::SQLite)),
    signature_comment: false,
    color: false,
};
let sql = compile(&prql, opts).unwrap();
assert_eq!("SELECT name, age FROM employees", sql);
```
