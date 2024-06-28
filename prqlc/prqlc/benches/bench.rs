use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use glob::glob;
use prqlc::{compile, pl_to_prql, pl_to_rq, prql_to_pl, Options};
use std::collections::BTreeMap;
use std::fs;

type Queries = BTreeMap<String, String>;

fn load_queries() -> Queries {
    glob("tests/integration/queries/**/*.prql")
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?;
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let content = fs::read_to_string(&path).ok()?;
            Some((name, content))
        })
        .collect()
}

fn bench_compile(c: &mut Criterion) {
    let queries = load_queries();
    let options = Options::default();
    let mut group = c.benchmark_group("compile");

    for (name, content) in queries.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), content, |b, content| {
            b.iter(|| compile(content, &options));
        });
    }
    group.finish();
}

fn bench_prql_to_pl(c: &mut Criterion) {
    let queries = load_queries();
    let mut group = c.benchmark_group("prql_to_pl");

    for (name, content) in queries.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), content, |b, content| {
            b.iter(|| prql_to_pl(content));
        });
    }
    group.finish();
}

fn bench_pl_to_rq(c: &mut Criterion) {
    let queries = load_queries();
    let mut group = c.benchmark_group("pl_to_rq");

    for (name, content) in queries.iter() {
        let pl = prql_to_pl(content).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(name), &pl, |b, pl| {
            b.iter(|| pl_to_rq(pl.clone()));
        });
    }
    group.finish();
}

fn bench_pl_to_prql(c: &mut Criterion) {
    let queries = load_queries();
    let mut group = c.benchmark_group("pl_to_prql");

    for (name, content) in queries.iter() {
        let pl = prql_to_pl(content).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(name), &pl, |b, pl| {
            b.iter(|| pl_to_prql(pl));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_compile,
    bench_prql_to_pl,
    bench_pl_to_rq,
    bench_pl_to_prql
);
criterion_main!(benches);
