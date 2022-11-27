use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use prql_compiler::{parse, pl_to_json};

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
    let rs_sql_str: String =
        prql_compiler::compile(&prql_query).expect("Couldn't compile query to prql!");
    env.new_string(rs_sql_str)
        .expect("Couldn't create java string!")
        .into_raw()
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
    let rs_json_str: String = {
        let prql: &str = &prql_query;
        parse(prql).and_then(pl_to_json)
    }
    .expect("Couldn't get prql json of query!");
    env.new_string(rs_json_str)
        .expect("Couldn't create java string!")
        .into_raw()
}
