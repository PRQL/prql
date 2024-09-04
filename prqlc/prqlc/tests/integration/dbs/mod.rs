#![cfg(not(target_family = "wasm"))]
#![cfg(any(feature = "test-dbs", feature = "test-dbs-external"))]

mod protocol;
pub(crate) mod runner;

pub type Row = Vec<String>;
