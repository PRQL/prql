
cargo test --no-run --package prql-compiler --test=sql -- sql::long_query

ulimit -s 1024

cargo test --package prql-compiler --test=sql -- sql::long_query
