#![cfg(not(target_family = "wasm"))]

extern crate libc;

use libc::{c_char, c_int};
use prql_compiler::{Options, Target};
use prql_compiler::{json, prql_to_pl};
use std::ffi::CStr;
use std::ffi::CString;
use std::str::FromStr;

#[repr(C)]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target and dialect to compile to.
    pub target: *const c_char,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

impl From<CompileOptions> for Options {
    fn from(o: CompileOptions) -> Self {
        let target_str = if o.target.is_null() {
            let c_str = unsafe { CStr::from_ptr(o.target) };
            c_str.to_str().unwrap()
        } else {
            ""
        };

        let target = Target::from_str(&target_str).unwrap_or_default();

        Options {
            format: o.format,
            target,
            signature_comment: o.signature_comment,
        }
    }
}

#[no_mangle]
//#[allow(non_snake_case)]
/// # Safety
///
/// This function is inherently unsafe because it is using C ABI.
pub unsafe extern "C" fn compile(query: *const c_char, options: CompileOptions) -> *const c_char {
    let prql_query: String = CStr::from_ptr(query).to_string_lossy().into_owned();

    let result = match prql_compiler::compile(&prql_query, &Options::from(options)) {
        Ok(sql_str) => sql_str,
        Err(_) => "".to_string()
    };

    let c_string = CString::new(result).unwrap();

    c_string.into_raw()
}

#[no_mangle]
#[allow(non_snake_case)]
/// # Safety
///
/// This function is inherently unsafe because it is using C ABI.
pub unsafe extern "C" fn to_sql(query: *const c_char, out: *mut c_char) -> c_int {
    let prql_query: String = CStr::from_ptr(query).to_string_lossy().into_owned();

    let (isErr, sql_result) = match prql_compiler::compile(&prql_query, &Options::default()) {
        Ok(sql_str) => (false, sql_str),
        Err(err) => {
            //let err_str = format!("{}", err);
            (true, err.to_string())
        }
    };

    let copylen = sql_result.len();
    let c_str = CString::new(sql_result).unwrap();

    out.copy_from(c_str.as_ptr(), copylen);
    let end_of_string_ptr = out.add(copylen);
    *end_of_string_ptr = 0;

    match isErr {
        true => -1,
        false => 0,
    }
}

#[no_mangle]
#[allow(non_snake_case)]
/// # Safety
///
/// This function is inherently unsafe because it using C ABI.
pub unsafe extern "C" fn to_json(query: *const c_char, out: *mut c_char) -> c_int {
    let prql_query: String = CStr::from_ptr(query).to_string_lossy().into_owned();

    let (isErr, sql_result) = match prql_to_pl(&prql_query).and_then(json::from_pl) {
        Ok(sql_str) => (false, sql_str),
        Err(err) => {
            //let err_str = format!("{}", err);
            (true, err.to_string())
        }
    };

    let copylen = sql_result.len();
    let c_str = CString::new(sql_result).unwrap();

    out.copy_from(c_str.as_ptr(), copylen);
    let end_of_string_ptr = out.add(copylen);
    *end_of_string_ptr = 0;

    match isErr {
        true => -1,
        false => 0,
    }
}
