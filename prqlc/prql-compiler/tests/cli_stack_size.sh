
cargo test --no-run --package prqlc --test=test

ulimit -s 1024

cargo test --package prqlc --test=test -- long_query
