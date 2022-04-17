#![allow(clippy::unused_unit)]
mod utils;

use prql_compiler::format_error;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is how you call js functions
// #[wasm_bindgen]
// extern {
//     fn compile(s: &str);
// }

#[wasm_bindgen]
pub fn compile(s: &str) -> CompileResult {
    let result = prql_compiler::compile(s).map_err(|e| format_error(e, "", s, false));

    // I had to make new CompileResult struct, because I couldn't make wasm_bindgen
    // accept it as a function return value. I also had to implement a few getters. Yuck.
    let mut r = CompileResult::default();
    match result {
        Ok(sql) => r.sql = Some(sql),
        Err(e) => {
            r.error = Some(CompileError {
                message: e.0,
                location: e.1.map(|l| SourceLocation {
                    start_line: l.start.0,
                    start_column: l.start.1,
                    end_line: l.end.0,
                    end_column: l.end.1,
                }),
            })
        }
    }
    r
}

#[wasm_bindgen]
#[derive(Default)]
pub struct CompileResult {
    sql: Option<String>,
    error: Option<CompileError>,
}

#[wasm_bindgen]
impl CompileResult {
    #[wasm_bindgen(getter)]
    pub fn sql(&self) -> Option<String> {
        self.sql.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<CompileError> {
        self.error.clone()
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct CompileError {
    message: String,
    location: Option<SourceLocation>,
}

#[wasm_bindgen]
impl CompileError {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn location(&self) -> Option<SourceLocation> {
        self.location.clone()
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct SourceLocation {
    pub start_line: usize,
    pub start_column: usize,

    pub end_line: usize,
    pub end_column: usize,
}
