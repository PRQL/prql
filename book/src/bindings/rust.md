# Rust (prql-compiler)

## Installation

```
cargo new myproject
cd myproject
cargo add prql-compiler
```

## Usage

`cargo run`

#### src/main.rs

```rust
use prql_compiler::compile;
use prql_compiler::sql;

fn main() {
    let prql = "from employees | select [name,age]  ";
    let opt = sql::Options {
             format: true,
             dialect: Some(sql::Dialect::SQLite),
             signature_comment: true,
         };
    let sql = compile(&prql, Some(opt)).unwrap();
    println!("PRQL: {}\nSQLite: {}", prql, sql);
}

```

#### Cargo.toml

```
[package]
name = "myproject"
version = "0.1.0"
edition = "2021"

[dependencies]
prql-compiler = "0.4.0"
```
