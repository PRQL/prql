// https://github.com/rustwasm/wasm-bindgen/pull/2984
#![allow(clippy::drop_non_drop)]
mod utils;

use prql_compiler::{format_error, pl_to_json, pl_to_prql, translate, FormattedError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile(s: &str) -> CompileResult {
    let result = prql_compiler::compile(s);

    // I had to make new CompileResult struct, because I couldn't make wasm_bindgen
    // accept it as a function return value. I also had to implement a few getters. Yuck.
    let mut r = CompileResult::default();
    match result {
        Ok(sql) => r.sql = Some(sql),
        Err(e) => {
            let error = format_error(e, "", s, false);

            r.error = Some(CompileError {
                line: error.line,
                message: error.message,
                location: error.location.map(|l| SourceLocation {
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
    line: String,
    message: String,
    location: Option<SourceLocation>,
}

#[wasm_bindgen]
impl CompileError {
    #[wasm_bindgen(getter)]
    pub fn line(&self) -> String {
        self.line.clone()
    }

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

#[wasm_bindgen]
pub fn to_sql(s: &str) -> Option<String> {
    let result = prql_compiler::compile(s).map_err(|e| format_error(e, "", s, false));
    return_or_throw_error(result)
}

#[wasm_bindgen]
pub fn to_json(s: &str) -> Option<String> {
    let result = prql_compiler::parse(s)
        .and_then(pl_to_json)
        .map_err(|e| format_error(e, "", s, false));
    return_or_throw_error(result)
}

#[wasm_bindgen]
pub fn from_json(s: &str) -> Option<String> {
    let result = prql_compiler::json_to_pl(s)
        .and_then(pl_to_prql)
        .map_err(|e| format_error(e, "", s, false));
    return_or_throw_error(result)
}

#[wasm_bindgen]
pub fn json_to_rq_to_sql(s: &str) -> Option<String> {
    let result = prql_compiler::json_to_rq(s)
        .and_then(translate)
        .map_err(|e| format_error(e, "", s, false));
    return_or_throw_error(result)
}

fn return_or_throw_error(result: Result<String, FormattedError>) -> Option<String> {
    match result {
        Ok(sql) => Some(sql),
        Err(e) => {
            wasm_bindgen::throw_str(&e.message);
        }
    }
}
