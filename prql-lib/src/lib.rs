#![cfg(not(target_family = "wasm"))]

extern crate libc;

use libc::{c_char, size_t};
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
/// This function assumes zero-terminated input strings.
#[no_mangle]
pub unsafe extern "C" fn compile(
    prql_query: *const c_char,
    options: *const Options,
) -> CompileResult {
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

    result_into_c_str(result)
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
pub unsafe extern "C" fn prql_to_pl(prql_query: *const c_char) -> CompileResult {
    let prql_query: String = c_str_to_string(prql_query);

    let result = Ok(prql_query.as_str())
        .and_then(prql_compiler::prql_to_pl)
        .and_then(prql_compiler::json::from_pl);
    result_into_c_str(result)
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
pub unsafe extern "C" fn pl_to_rq(pl_json: *const c_char) -> CompileResult {
    let pl_json: String = c_str_to_string(pl_json);

    let result = Ok(pl_json.as_str())
        .and_then(prql_compiler::json::to_pl)
        .and_then(prql_compiler::pl_to_rq)
        .and_then(prql_compiler::json::from_rq);
    result_into_c_str(result)
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
pub unsafe extern "C" fn rq_to_sql(rq_json: *const c_char) -> CompileResult {
    let rq_json: String = c_str_to_string(rq_json);

    let result = Ok(rq_json.as_str())
        .and_then(prql_compiler::json::to_rq)
        .and_then(|x| prql_compiler::rq_to_sql(x, &prql_compiler::Options::default()));
    result_into_c_str(result)
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

#[repr(C)]
pub struct CompileResult {
    pub output: *const i8,
    pub errors: *const ErrorMessage,
    pub errors_len: size_t,
}

/// An error message.
///
/// Calling code is responsible for freeing all memory allocated
/// for fields as well as strings.
// Make sure to keep in sync with prql_compiler::ErrorMessage
#[repr(C)]
pub struct ErrorMessage {
    /// Machine-readable identifier of the error
    pub code: *const *const i8,
    /// Plain text of the error
    pub reason: *const i8,
    /// A list of suggestions of how to fix the error
    pub hint: *const *const i8,
    /// Character offset of error origin within a source file
    pub span: *const Span,

    /// Annotated code, containing cause and hints.
    pub display: *const *const i8,
    /// Line and column number of error origin within a source file
    pub location: *const SourceLocation,
}

// Make sure to keep in sync with prql_compiler::Span
#[repr(C)]
pub struct Span {
    pub start: size_t,
    pub end: size_t,
}

/// Location within the source file.
/// Tuples contain:
/// - line number (0-based),
/// - column number within that line (0-based),
///
// Make sure to keep in sync with prql_compiler::SourceLocation
#[repr(C)]
pub struct SourceLocation {
    pub start: (size_t, size_t),

    pub end: (size_t, size_t),
}

unsafe fn result_into_c_str(result: Result<String, ErrorMessages>) -> CompileResult {
    match result {
        Ok(output) => CompileResult {
            output: convert_string(output),
            errors: ::std::ptr::null_mut(),
            errors_len: 0,
        },
        Err(err) => {
            let mut errors = Vec::with_capacity(err.inner.len());
            errors.extend(err.inner.into_iter().map(|e| ErrorMessage {
                code: option_to_ptr(e.code.map(convert_string)),
                reason: convert_string(e.reason),
                hint: option_to_ptr(e.hint.map(convert_string)),
                span: option_to_ptr(e.span.map(convert_span)),
                display: option_to_ptr(e.display.map(convert_string)),
                location: option_to_ptr(e.location.map(convert_source_location)),
            }));
            CompileResult {
                output: CString::default().into_raw(),
                errors_len: errors.len(),
                errors: errors.leak().as_ptr(),
            }
        }
    }
}

fn option_to_ptr<T>(o: Option<T>) -> *const T {
    match o {
        Some(x) => {
            let b = Box::new(x);
            Box::into_raw(b)
        },
        None => ::std::ptr::null(),
    }
}

fn convert_string(x: String) -> *const i8 {
    CString::new(x).unwrap_or_default().into_raw()
}

fn convert_span(x: prql_compiler::Span) -> Span {
    Span {
        start: x.start,
        end: x.end,
    }
}

fn convert_source_location(x: prql_compiler::SourceLocation) -> SourceLocation {
    SourceLocation {
        start: x.start,
        end: x.end,
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
