#![cfg(target_family = "wasm")]

use std::str::FromStr;

use prql_compiler::Target;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> Option<String> {
    return_or_throw(
        prql_compiler::compile(prql_query, &options.map(|x| x.into()).unwrap_or_default())
            .map_err(|e| e.composed(&prql_query.into())),
    )
}

#[wasm_bindgen]
pub fn prql_to_pl(prql_query: &str) -> Option<String> {
    return_or_throw(
        Ok(prql_query)
            .and_then(prql_compiler::prql_to_pl)
            .and_then(prql_compiler::json::from_pl),
    )
}

#[wasm_bindgen]
pub fn pl_to_rq(pl_json: &str) -> Option<String> {
    return_or_throw(
        Ok(pl_json)
            .and_then(prql_compiler::json::to_pl)
            .and_then(prql_compiler::pl_to_rq)
            .and_then(prql_compiler::json::from_rq),
    )
}

#[wasm_bindgen]
pub fn rq_to_sql(rq_json: &str) -> Option<String> {
    return_or_throw(
        Ok(rq_json)
            .and_then(prql_compiler::json::to_rq)
            .and_then(|x| prql_compiler::rq_to_sql(x, &prql_compiler::Options::default())),
    )
}

/// Compilation options for SQL backend of the compiler.
#[wasm_bindgen]
#[derive(Clone)]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    #[wasm_bindgen(skip)]
    pub target: String,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

#[wasm_bindgen]
pub fn get_targets() -> Vec<JsValue> {
    prql_compiler::Target::names()
        .iter()
        .map(|t| JsValue::from_str(t))
        .collect()
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            format: true,
            target: String::new(),
            signature_comment: true,
        }
    }
}

#[wasm_bindgen]
impl CompileOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Target to compile to (e.g. sql.postgres)
    ///
    /// Defaults to `sql.any`, which uses `target` argument from the query header to determine
    /// the SQL dialect.
    #[wasm_bindgen(getter)]
    pub fn target(&self) -> String {
        self.target.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_target(&mut self, target: String) {
        self.target = target;
    }
}

impl From<CompileOptions> for prql_compiler::Options {
    fn from(o: CompileOptions) -> Self {
        let target = Target::from_str(&o.target).unwrap_or_default();

        prql_compiler::Options {
            format: o.format,
            target,
            signature_comment: o.signature_comment,
            // TODO: offer this option in the API
            color: false,
        }
    }
}

fn return_or_throw(result: Result<String, prql_compiler::ErrorMessages>) -> Option<String> {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics. See
    // `Cargo.toml` for notes.
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    match result {
        Ok(sql) => Some(sql),
        Err(e) => wasm_bindgen::throw_str(&e.to_json()),
    }
}
