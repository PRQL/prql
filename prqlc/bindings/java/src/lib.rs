use std::str::FromStr;

use jni::errors::{Error as JniError, Result as JniResult, ThrowRuntimeExAndDefault};
use jni::objects::{JClass, JString};
use jni::strings::JNIString;
use jni::sys::jboolean;
use jni::{jni_str, Env, EnvUnowned};
use prqlc::{json, pl_to_prql, prql_to_pl, ErrorMessages, Options, Target};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toSql<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    query: JString<'local>,
    target: JString<'local>,
    format: jboolean,
    signature: jboolean,
) -> JString<'local> {
    env.with_env(|env| -> JniResult<JString<'local>> {
        let prql_query = jstring_to_string(env, &query, "query")?;
        let target_str = jstring_to_string(env, &target, "target")?;
        let prql_dialect = match Target::from_str(&target_str) {
            Ok(t) => t,
            Err(e) => {
                return Err(throw_illegal_argument(
                    env,
                    &format!("invalid target dialect: {e}"),
                ))
            }
        };
        let opt = Options {
            format,
            target: prql_dialect,
            signature_comment: signature,
            // TODO: add support for `display`
            ..Default::default()
        };
        let result = prqlc::compile(&prql_query, &opt);
        java_string_with_exception(result, env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_format<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    query: JString<'local>,
) -> JString<'local> {
    env.with_env(|env| -> JniResult<JString<'local>> {
        let prql_query = jstring_to_string(env, &query, "query")?;
        let result = prql_to_pl(&prql_query).and_then(|x| pl_to_prql(&x));
        java_string_with_exception(result, env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_org_prql_prql4j_PrqlCompiler_toJson<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    query: JString<'local>,
) -> JString<'local> {
    env.with_env(|env| -> JniResult<JString<'local>> {
        let prql_query = jstring_to_string(env, &query, "query")?;
        let result = prql_to_pl(&prql_query).and_then(|x| json::from_pl(&x));
        java_string_with_exception(result, env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

fn jstring_to_string(env: &mut Env, s: &JString, name: &str) -> JniResult<String> {
    match s.try_to_string(env) {
        Ok(text) => Ok(text),
        Err(e) => Err(throw_illegal_argument(
            env,
            &format!("failed to read {name}: {e}"),
        )),
    }
}

/// Throws an `IllegalArgumentException` and returns the error to propagate out
/// of the `with_env` closure. The error policy leaves an already-pending
/// exception alone, so the exception thrown here is the one Java observes.
fn throw_illegal_argument(env: &mut Env, message: &str) -> JniError {
    match env.throw_new(
        jni_str!("java/lang/IllegalArgumentException"),
        JNIString::from(message),
    ) {
        // `throw_new` reports `Err(Error::JavaException)` once the exception is
        // pending; anything else means the throw itself failed.
        Ok(()) => JniError::JavaException,
        Err(e) => e,
    }
}

fn java_string_with_exception<'local>(
    result: Result<String, ErrorMessages>,
    env: &mut Env<'local>,
) -> JniResult<JString<'local>> {
    match result {
        Ok(text) => match env.new_string(text) {
            Ok(js) => Ok(js),
            Err(e) => Err(throw_illegal_argument(
                env,
                &format!("failed to create java string: {e}"),
            )),
        },
        Err(err) => {
            let message = err.to_string();
            let exception = env.find_class(jni_str!("java/lang/Exception"))?;
            Err(match env.throw_new(exception, JNIString::from(message)) {
                Ok(()) => JniError::JavaException,
                Err(e) => e,
            })
        }
    }
}
