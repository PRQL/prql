use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use prqlc::{json, pl_to_prql, prql_to_pl, ErrorMessages, Options, Target};
use std::str::FromStr;

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
    let prql_query: String = env
        .get_string(&query)
        .expect("Couldn't get java string!")
        .into();
    let target_str: String = env
        .get_string(&target)
        .expect("Couldn't get java string")
        .into();
    let prql_dialect: Target = Target::from_str(&target_str).unwrap_or(Target::Sql(None));
    let opt = Options {
        format: format != 0,
        target: prql_dialect,
        signature_comment: signature != 0,
        // TODO: add support for this
        color: false,
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
    let prql_query: String = env
        .get_string(&query)
        .expect("Couldn't get java string!")
        .into();
    let result = prql_to_pl(&prql_query).and_then(pl_to_prql);
    java_string_with_exception(result, &mut env)
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toJson(
    mut env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query: String = env
        .get_string(&query)
        .expect("Couldn't get java string!")
        .into();
    let result = prql_to_pl(&prql_query).and_then(json::from_pl);
    java_string_with_exception(result, &mut env)
}

fn java_string_with_exception(result: Result<String, ErrorMessages>, env: &mut JNIEnv) -> jstring {
    if let Ok(text) = result {
        env.new_string(text)
            .expect("Couldn't create java string!")
            .into_raw()
    } else {
        let exception = env.find_class("java/lang/Exception").unwrap();
        if let Err(e) = env.throw_new(exception, result.err().unwrap().to_string()) {
            println!("Error throwing exception: {:?}", e);
        }
        std::ptr::null_mut() as jstring
    }
}
