#![cfg(not(target_family = "wasm"))]

extern crate libc;

use libc::{c_char, size_t};
use prqlc::ErrorMessages;
use prqlc::Target;
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
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
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
                .and_then(prqlc::prql_to_pl)
                .and_then(prqlc::pl_to_rq)
                .and_then(|rq| prqlc::rq_to_sql(rq, &opts.unwrap_or_default()))
        })
        .map_err(|e| e.composed(&prql_query.into()));

    result_into_c_str(result)
}

/// Build PL AST from a PRQL string. PL in documented in the
/// [prqlc Rust crate](https://docs.rs/prqlc/latest/prqlc/ir/pl).
///
/// Takes PRQL source buffer and writes PL serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
#[no_mangle]
pub unsafe extern "C" fn prql_to_pl(prql_query: *const c_char) -> CompileResult {
    let prql_query: String = c_str_to_string(prql_query);

    let result = Ok(prql_query.as_str())
        .and_then(prqlc::prql_to_pl)
        .and_then(|x| prqlc::json::from_pl(&x));
    result_into_c_str(result)
}

/// Finds variable references, validates functions calls, determines frames and converts PL to RQ.
/// PL and RQ are documented in the
/// [prqlc Rust crate](https://docs.rs/prqlc/latest/prqlc/ast).
///
/// Takes PL serialized as JSON buffer and writes RQ serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
#[no_mangle]
pub unsafe extern "C" fn pl_to_rq(pl_json: *const c_char) -> CompileResult {
    let pl_json: String = c_str_to_string(pl_json);

    let result = Ok(pl_json.as_str())
        .and_then(prqlc::json::to_pl)
        .and_then(prqlc::pl_to_rq)
        .and_then(|x| prqlc::json::from_rq(&x));
    result_into_c_str(result)
}

/// Convert RQ AST into an SQL string. RQ is documented in the
/// [prqlc Rust crate](https://docs.rs/prqlc/latest/prqlc/ir/rq).
///
/// Takes RQ serialized as JSON buffer and writes SQL source to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
#[no_mangle]
pub unsafe extern "C" fn rq_to_sql(
    rq_json: *const c_char,
    options: *const Options,
) -> CompileResult {
    let rq_json: String = c_str_to_string(rq_json);

    let options = options.as_ref().map(convert_options).transpose();

    let result = options.and_then(|options| {
        Ok(rq_json.as_str())
            .and_then(prqlc::json::to_rq)
            .and_then(|x| prqlc::rq_to_sql(x, &options.unwrap_or_default()))
    });
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

/// Result of compilation.
#[repr(C)]
pub struct CompileResult {
    pub output: *const libc::c_char,
    pub messages: *const Message,
    pub messages_len: size_t,
}

/// Compile message kind. Currently only Error is implemented.
#[repr(C)]
pub enum MessageKind {
    Error,
    Warning,
    Lint,
}

/// Compile result message.
///
/// Calling code is responsible for freeing all memory allocated
/// for fields as well as strings.
// Make sure to keep in sync with prqlc::ErrorMessage
#[repr(C)]
pub struct Message {
    /// Message kind. Currently only Error is implemented.
    pub kind: MessageKind,
    /// Machine-readable identifier of the error
    pub code: *const *const libc::c_char,
    /// Plain text of the error
    pub reason: *const libc::c_char,
    /// A list of suggestions of how to fix the error
    pub hint: *const *const libc::c_char,
    /// Character offset of error origin within a source file
    pub span: *const Span,

    /// Annotated code, containing cause and hints.
    pub display: *const *const libc::c_char,
    /// Line and column number of error origin within a source file
    pub location: *const SourceLocation,
}

/// Identifier of a location in source.
/// Contains offsets in terms of chars.
// Make sure to keep in sync with prqlc::Span
#[repr(C)]
pub struct Span {
    pub start: size_t,
    pub end: size_t,
}

/// Location within a source file.
// Make sure to keep in sync with prqlc::SourceLocation
#[repr(C)]
pub struct SourceLocation {
    pub start_line: size_t,
    pub start_col: size_t,

    pub end_line: size_t,
    pub end_col: size_t,
}

/// Destroy a `CompileResult` once you are done with it.
///
/// # Safety
///
/// This function expects to be called exactly once after the call of any the functions
/// that return `CompileResult`. No fields should be freed manually.
#[no_mangle]
pub unsafe extern "C" fn result_destroy(res: CompileResult) {
    // This is required because we are allocating memory for
    // strings, vectors and options.
    // For strings and vectors this is required, but options may be
    // able to live entirely within the struct, instead of the heap.

    for i in 0..res.messages_len {
        let e = &*res.messages.add(i);

        if !e.code.is_null() {
            drop(CString::from_raw(*e.code as *mut libc::c_char));
            drop(Box::from_raw(e.code as *mut *const libc::c_char));
        }
        drop(CString::from_raw(e.reason as *mut libc::c_char));
        if !e.hint.is_null() {
            drop(CString::from_raw(*e.hint as *mut libc::c_char));
            drop(Box::from_raw(e.hint as *mut *const libc::c_char));
        }
        if !e.span.is_null() {
            drop(Box::from_raw(e.span as *mut Span));
        }
        if !e.display.is_null() {
            drop(CString::from_raw(*e.display as *mut libc::c_char));
            drop(Box::from_raw(e.display as *mut *const libc::c_char));
        }
        if !e.location.is_null() {
            drop(Box::from_raw(e.location as *mut SourceLocation));
        }
    }
    drop(Vec::from_raw_parts(
        res.messages as *mut i8,
        res.messages_len,
        res.messages_len,
    ));
    drop(CString::from_raw(res.output as *mut libc::c_char));
}

unsafe fn result_into_c_str(result: Result<String, ErrorMessages>) -> CompileResult {
    match result {
        Ok(output) => CompileResult {
            output: convert_string(output),
            messages: ::std::ptr::null_mut(),
            messages_len: 0,
        },
        Err(err) => {
            let mut errors = Vec::with_capacity(err.inner.len());
            errors.extend(err.inner.into_iter().map(|e| Message {
                kind: MessageKind::Error,
                code: option_to_ptr(e.code.map(convert_string)),
                reason: convert_string(e.reason),
                hint: option_to_ptr(if e.hints.is_empty() {
                    None
                } else {
                    Some(convert_string(e.hints.join("\n")))
                }),
                span: option_to_ptr(e.span.map(convert_span)),
                display: option_to_ptr(e.display.map(convert_string)),
                location: option_to_ptr(e.location.map(convert_source_location)),
            }));
            CompileResult {
                output: CString::default().into_raw(),
                messages_len: errors.len(),
                messages: errors.leak().as_ptr(),
            }
        }
    }
}

/// Allocates the value on the heap and returns a pointer to it.
/// If the input is None, it returns null pointer.
fn option_to_ptr<T>(o: Option<T>) -> *const T {
    match o {
        Some(x) => {
            let b = Box::new(x);
            Box::into_raw(b)
        }
        None => ::std::ptr::null(),
    }
}

fn convert_string(x: String) -> *const libc::c_char {
    CString::new(x).unwrap_or_default().into_raw()
}

fn convert_span(x: prqlc::Span) -> Span {
    Span {
        start: x.start,
        end: x.end,
    }
}

fn convert_source_location(x: prqlc::SourceLocation) -> SourceLocation {
    SourceLocation {
        start_line: x.start.0,
        start_col: x.start.1,
        end_line: x.end.0,
        end_col: x.end.1,
    }
}

unsafe fn c_str_to_string(c_str: *const c_char) -> String {
    // inefficient, but simple
    CStr::from_ptr(c_str).to_string_lossy().into_owned()
}

fn convert_options(o: &Options) -> Result<prqlc::Options, prqlc::ErrorMessages> {
    let target = if !o.target.is_null() {
        Some(unsafe { c_str_to_string(o.target) })
    } else {
        None
    };
    let target = target
        .as_deref()
        .filter(|x| !x.is_empty())
        .unwrap_or("sql.any");

    let target = Target::from_str(target).map_err(|e| prqlc::downcast(e.into()))?;

    Ok(prqlc::Options {
        format: o.format,
        target,
        signature_comment: o.signature_comment,
        // TODO: add support for this
        color: false,
    })
}
