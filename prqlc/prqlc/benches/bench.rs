//! A very simple benchmark for a single example, basically copied from the
//! [criterion quick
//! start](https://github.com/bheisler/criterion.rs#quickstart). At time of
//! writing, this ran at 300mics on my laptop.

// TODO: add a small GHA workflow for this to run on "full tests".

// While benchmarks could run on wasm, the default features use Rayon, which
// isn't supported. We're hardly using benchmarks so it's fine to exclude one
// target.

cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        fn main() {    panic!("Not used in wasm (but it seems cargo insists we have a `main` function).")}
    } else {
        use criterion::{criterion_group, criterion_main, Criterion};
        use prqlc::{ErrorMessages, Options, compile};

        const CONTENT: &str = include_str!("../examples/compile-files/queries/variables.prql");
        fn compile_query() -> Result<String, ErrorMessages> {
            compile(CONTENT, &Options::default())
        }

        fn criterion_benchmark(c: &mut Criterion) {
            c.bench_function("variables-query", |b| b.iter(compile_query));
        }

        criterion_group!(benches, criterion_benchmark);
        criterion_main!(benches);

    }
}
