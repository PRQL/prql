use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use prql_compiler::{json, prql_to_pl, pl_to_prql, Options, ErrorMessages};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toSql(
    env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query: String = env
        .get_string(query)
        .expect("Couldn't get java string!")
        .into();
    let result = prql_compiler::compile(&prql_query, &Options::default());
    return java_string_with_exception(result, &env);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_format(
    env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query: String = env
        .get_string(query)
        .expect("Couldn't get java string!")
        .into();
    let result = prql_to_pl(&prql_query).and_then(pl_to_prql);
    return java_string_with_exception(result,&env);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toJson(
    env: JNIEnv,
    _class: JClass,
    query: JString,
) -> jstring {
    let prql_query: String = env
        .get_string(query)
        .expect("Couldn't get java string!")
        .into();
    let result = prql_to_pl(&prql_query).and_then(json::from_pl);
    return java_string_with_exception(result, &env);
}

fn java_string_with_exception(result: Result<String, ErrorMessages>, env: &JNIEnv) -> jstring {
    if result.is_ok() {
        env.new_string(result.unwrap())
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
