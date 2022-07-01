#![allow(clippy::unused_unit)]

mod utils;

use prql_compiler::format_error;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn compile(s: &str) -> Option<String> {
    let result = prql_compiler::compile(s).map_err(|e| format_error(e, "", s, false));
    match result {
        Ok(sql) => Some(sql),
        Err(e) => wasm_bindgen::throw_str(format!("{:?}", e.0).as_str()),
    }
}
