// Exclude benchmarks from WASM builds (criterion depends on alloca).
// The inner cfg attr compiles an empty crate, but we still need a main for WASM.
#![cfg_attr(not(target_family = "wasm"), allow(unused))]

#[cfg(target_family = "wasm")]
fn main() {}

#[cfg(not(target_family = "wasm"))]
include!("bench_impl.rs");
