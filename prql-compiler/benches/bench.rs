//! A very simple benchmark for a single example, basically copied from the
//! [criterion quick
//! start](https://github.com/bheisler/criterion.rs#quickstart). At time of
//! writing, this ran at 300mics on my laptop.

// TODO: add a small GHA workflow for this to run on "full tests".

use criterion::{criterion_group, criterion_main, Criterion};
use prql_compiler::*;

const CONTENT: &str = include_str!("../../book/tests/prql/examples/variables-0.prql");
fn compile_query() -> Result<String> {
    compile(CONTENT)
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("variables-query", |b| b.iter(compile_query));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
