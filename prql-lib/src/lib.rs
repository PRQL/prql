#![cfg(not(target_family = "wasm"))]

extern crate libc;

use libc::{c_char, c_int};
use prql_compiler::ErrorMessages;
use prql_compiler::Target;
use std::ffi::CStr;
use std::ffi::CString;
use std::str::FromStr;

/// Compile a PRQL string into a SQL string.
///
/// This is a wrapper for: `prql_to_pl`, `pl_to_rq` and `rq_to_sql` without converting to JSON
/// between each of the functions.
///
/// See `Options` struct for available compilation options.
///
/// # Safety
///
/// This function assumes zero-terminated strings and sufficiently large output buffers.
#[no_mangle]
pub unsafe extern "C" fn compile(
    prql_query: *const c_char,
    options: *const Options,
    out: *mut c_char,
) -> c_int {
    let prql_query: String = c_str_to_string(prql_query);

    let options = options.as_ref().map(convert_options).transpose();

    let result = options
        .and_then(|opts| {
            Ok(prql_query.as_str())
                .and_then(prql_compiler::prql_to_pl)
                .and_then(prql_compiler::pl_to_rq)
                .and_then(|rq| prql_compiler::rq_to_sql(rq, &opts.unwrap_or_default()))
        })
        .map_err(|e| e.composed("", &prql_query, false));

    result_into_c_str(result, out)
}

/// Build PL AST from a PRQL string. PL in documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast/pl).
///
/// Takes PRQL source buffer and writes PL serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated strings and sufficiently large output buffers.
#[no_mangle]
pub unsafe extern "C" fn prql_to_pl(prql_query: *const c_char, out: *mut c_char) -> c_int {
    let prql_query: String = c_str_to_string(prql_query);

    let result = Ok(prql_query.as_str())
        .and_then(prql_compiler::prql_to_pl)
        .and_then(prql_compiler::json::from_pl);
    result_into_c_str(result, out)
}

/// Finds variable references, validates functions calls, determines frames and converts PL to RQ.
/// PL and RQ are documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast).
///
/// Takes PL serialized as JSON buffer and writes RQ serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated strings and sufficiently large output buffers.
#[no_mangle]
pub unsafe extern "C" fn pl_to_rq(pl_json: *const c_char, out: *mut c_char) -> c_int {
    let pl_json: String = c_str_to_string(pl_json);

    let result = Ok(pl_json.as_str())
        .and_then(prql_compiler::json::to_pl)
        .and_then(prql_compiler::pl_to_rq)
        .and_then(prql_compiler::json::from_rq);
    result_into_c_str(result, out)
}

/// Convert RQ AST into an SQL string. RQ is documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast/rq).
///
/// Takes RQ serialized as JSON buffer and writes SQL source to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated strings and sufficiently large output buffers.
#[no_mangle]
pub unsafe extern "C" fn rq_to_sql(rq_json: *const c_char, out: *mut c_char) -> c_int {
    let rq_json: String = c_str_to_string(rq_json);

    let result = Ok(rq_json.as_str())
        .and_then(prql_compiler::json::to_rq)
        .and_then(|x| prql_compiler::rq_to_sql(x, &prql_compiler::Options::default()));
    result_into_c_str(result, out)
}

/// Compilation options
#[repr(C)]
pub struct Options {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target and dialect to compile to.
    ///
    /// Defaults to `sql.any`, which uses `target` argument from the query header to determine
    /// the SQL dialect.
    pub target: *mut c_char,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

unsafe fn result_into_c_str(result: Result<String, ErrorMessages>, out: *mut c_char) -> i32 {
    let (is_err, string) = match result {
        Ok(string) => (false, string),
        Err(err) => (true, err.to_string()),
    };

    let copy_len = string.bytes().len();
    let c_str = CString::new(string).unwrap();

    out.copy_from(c_str.as_ptr(), copy_len);
    let end_of_string_ptr = out.add(copy_len);
    *end_of_string_ptr = 0;

    match is_err {
        true => -1,
        false => 0,
    }
}

unsafe fn c_str_to_string(c_str: *const c_char) -> String {
    // inefficient, but simple
    CStr::from_ptr(c_str).to_string_lossy().into_owned()
}

fn convert_options(o: &Options) -> Result<prql_compiler::Options, prql_compiler::ErrorMessages> {
    let target = if o.target.is_null() {
        Some(unsafe { c_str_to_string(o.target) })
    } else {
        None
    };
    let target = target
        .as_deref()
        .filter(|x| !x.is_empty())
        .unwrap_or("sql.any");

    let target = Target::from_str(target).map_err(|e| prql_compiler::downcast(e.into()))?;

    Ok(prql_compiler::Options {
        format: o.format,
        target,
        signature_comment: o.signature_comment,
    })
}
