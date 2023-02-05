# PRQL compiler

`prql-compiler` contains the implementation of PRQL's compiler, written in Rust.

For more on PRQL, check out the [PRQL website](https://prql-lang.org) or the
[PRQL repo](https://github.com/PRQL/prql).

For more usage examples and the library documentation, check out the
[`prql-compiler` documentation](https://docs.rs/prql-compiler/latest/prql_compiler/).

# Installation

```shell
cargo add prql-compiler
```

## Examples

Compile a PRQL string to a SQLite dialect string.

**src/main.rs**

```rust
use prql_compiler::{compile, sql};

let prql = "from employees | select [name, age]";
let opt = sql::Options{
    format: false,
    dialect: Some(sql::Dialect::SQLite),
    signature_comment: false
};
let sql = compile(&prql, Some(opt)).unwrap();
assert_eq!("SELECT name, age FROM employees", sql);
```

## Terminology

[_Relation_](<https://en.wikipedia.org/wiki/Relation_(database)>): Standard
definition of a relation in context of databases:

- An ordered set of tuples of form `(d_0, d_1, d_2, ...)`.
- Set of all `d_x` is called an attribute or a column. It has a name and a type
  domain `D_x`.

_Frame_: descriptor of a relation. Contains list of columns (with names and
types). Does not contain data.

[_Table_](<https://en.wikipedia.org/wiki/Table_(database)#Tables_versus_relations>):
persistently stored relation. Some uses of this term actually mean to say
"relation".
