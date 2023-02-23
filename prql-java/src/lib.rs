use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use prql_compiler::{json, prql_to_pl, Options};

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
    let rs_sql_str: String = prql_compiler::compile(&prql_query, &Options::default())
        .expect("Couldn't compile query to prql!");
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
    let rs_json_str: String = { prql_to_pl(&prql_query).and_then(json::from_pl) }
        .expect("Couldn't get json from prql query!");
    env.new_string(rs_json_str)
        .expect("Couldn't create java string!")
        .into_raw()
}
