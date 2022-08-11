#![cfg(not(target_family = "wasm"))]

extern crate libc;

use libc::{c_char, c_int};
use std::ffi::CStr;
use std::ffi::CString;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn to_sql(query: *const c_char, out: *mut c_char) -> c_int {
    let prql_query: String = unsafe { CStr::from_ptr(query).to_string_lossy().into_owned() };

    let (isErr, sql_result) = match prql_compiler::compile(&prql_query) {
        Ok(sql_str) => (false, sql_str),
        Err(err) => {
            //let err_str = format!("{}", err);
            (true, err.to_string())
        }
    };

    let copylen = sql_result.len();
    let c_str = CString::new(sql_result).unwrap();

    unsafe {
        out.copy_from(c_str.as_ptr(), copylen);
        let end_of_string_ptr = out.offset(copylen as isize);
        *end_of_string_ptr = 0;
    }

    return match isErr {
        true => -1,
        false => 0,
    };
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn to_json(query: *const c_char, out: *mut c_char) -> c_int {
    let prql_query: String = unsafe { CStr::from_ptr(query).to_string_lossy().into_owned() };

    let (isErr, sql_result) = match prql_compiler::to_json(&prql_query) {
        Ok(json_str) => (false, json_str),
        Err(err) => {
            let err_str = format!("{}", err);
            (true, err_str)
        }
    };

    let copylen = sql_result.len() + 1;
    let c_str = CString::new(sql_result).unwrap();

    unsafe { out.copy_from(c_str.as_ptr(), copylen) }
    if isErr {
        return -1;
    } else {
        return 0;
    };
}