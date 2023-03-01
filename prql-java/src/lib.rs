use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use prql_compiler::sql::Dialect;
use prql_compiler::{json, pl_to_prql, prql_to_pl, ErrorMessages, Options, Target};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toSql(
    env: JNIEnv,
    _class: JClass,
    query: JString,
    target: JString,
    format: jboolean,
    signature: jboolean,
) -> jstring {
    let prql_query: String = env
        .get_string(query)
        .expect("Couldn't get java string!")
        .into();
    let target_dialect: String = if let Ok(target_name) = env.get_string(target) {
        target_name.into()
    } else {
        "sql.generic".to_owned()
    };
    let prql_target = match target_dialect.as_str() {
        "sql.ansi" => Dialect::Ansi,
        "sql.bigquery" => Dialect::BigQuery,
        "sql.clickhouse" => Dialect::ClickHouse,
        "sql.duckdb" => Dialect::DuckDb,
        "sql.generic" => Dialect::Generic,
        "sql.hive" => Dialect::Hive,
        "sql.mssql" => Dialect::MsSql,
        "sql.mysql" => Dialect::MySql,
        "sql.postgres" => Dialect::PostgreSql,
        "sql.sqlite" => Dialect::SQLite,
        "sql.snowflake" => Dialect::Snowflake,
        _ => Dialect::Generic,
    };
    let opt = Options {
        format: format != 0,
        target: Target::Sql(Some(prql_target)),
        signature_comment: signature != 0,
    };
    let result = prql_compiler::compile(&prql_query, &opt);
    java_string_with_exception(result, &env)
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
    java_string_with_exception(result, &env)
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
    java_string_with_exception(result, &env)
}

fn java_string_with_exception(result: Result<String, ErrorMessages>, env: &JNIEnv) -> jstring {
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
