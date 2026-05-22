use std::str::FromStr;

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use prqlc::{json, pl_to_prql, prql_to_pl, ErrorMessages, Options, Target};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toSql(
    mut env: JNIEnv,
    _class: JClass,
    query: JString,
    target: JString,
    format: jboolean,
    signature: jboolean,
) -> jstring {
    let prql_query = match jstring_to_string(&mut env, &query, "query") {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let target_str = match jstring_to_string(&mut env, &target, "target") {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let prql_dialect = match Target::from_str(&target_str) {
        Ok(t) => t,
        Err(e) => {
            throw_illegal_argument(&mut env, &format!("invalid target dialect: {e}"));
            return std::ptr::null_mut();
        }
    };
    let opt = Options {
        format: format != 0,
        target: prql_dialect,
        signature_comment: signature != 0,
        // TODO: add support for `display`
        ..Default::default()
    };
    let result = prqlc::compile(&prql_query, &opt);
    java_string_with_exception(result, &mut env)
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_format(
    mut env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query = match jstring_to_string(&mut env, &query, "query") {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let result = prql_to_pl(&prql_query).and_then(|x| pl_to_prql(&x));
    java_string_with_exception(result, &mut env)
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toJson(
    mut env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query = match jstring_to_string(&mut env, &query, "query") {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let result = prql_to_pl(&prql_query).and_then(|x| json::from_pl(&x));
    java_string_with_exception(result, &mut env)
}

fn jstring_to_string(env: &mut JNIEnv, s: &JString, name: &str) -> Option<String> {
    match env.get_string(s) {
        Ok(js) => Some(js.into()),
        Err(e) => {
            throw_illegal_argument(env, &format!("failed to read {name}: {e}"));
            None
        }
    }
}

fn throw_illegal_argument(env: &mut JNIEnv, message: &str) {
    if let Err(e) = env.throw_new("java/lang/IllegalArgumentException", message) {
        eprintln!("Error throwing IllegalArgumentException: {e:?}");
    }
}

fn java_string_with_exception(result: Result<String, ErrorMessages>, env: &mut JNIEnv) -> jstring {
    match result {
        Ok(text) => match env.new_string(text) {
            Ok(js) => js.into_raw(),
            Err(e) => {
                throw_illegal_argument(env, &format!("failed to create java string: {e}"));
                std::ptr::null_mut()
            }
        },
        Err(err) => {
            let message = err.to_string();
            match env.find_class("java/lang/Exception") {
                Ok(exception) => {
                    if let Err(e) = env.throw_new(exception, message) {
                        eprintln!("Error throwing exception: {e:?}");
                    }
                }
                Err(e) => {
                    eprintln!("Error finding java/lang/Exception: {e:?}");
                }
            }
            std::ptr::null_mut()
        }
    }
}
