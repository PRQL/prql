#![cfg(not(target_family = "wasm"))]
use std::str::FromStr;

use prql_compiler::{self, sql::Dialect, IntoOnly, Target};
use pyo3::{exceptions, prelude::*};

#[pyfunction]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> PyResult<String> {
    Ok(prql_query)
        .and_then(prql_compiler::prql_to_pl)
        .and_then(prql_compiler::pl_to_rq)
        .and_then(|rq| prql_compiler::rq_to_sql(rq, options.map(|o| o.into()).unwrap_or_default()))
        .map_err(|e| e.composed("", prql_query, false))
        .map_err(|e| (PyErr::new::<exceptions::PySyntaxError, _>(e.into_only().unwrap().reason)))
}

#[pyfunction]
pub fn prql_to_pl(prql_query: &str) -> PyResult<String> {
    Ok(prql_query)
        .and_then(prql_compiler::prql_to_pl)
        .and_then(prql_compiler::json::from_pl)
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn pl_to_rq(pl_json: &str) -> PyResult<String> {
    Ok(pl_json)
        .and_then(prql_compiler::json::to_pl)
        .and_then(prql_compiler::pl_to_rq)
        .and_then(prql_compiler::json::from_rq)
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn rq_to_sql(rq_json: &str) -> PyResult<String> {
    Ok(rq_json)
        .and_then(prql_compiler::json::to_rq)
        .and_then(|x| prql_compiler::rq_to_sql(x, prql_compiler::Options::default()))
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pymodule]
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(prql_to_pl, m)?)?;
    m.add_function(wrap_pyfunction!(pl_to_rq, m)?)?;
    m.add_function(wrap_pyfunction!(rq_to_sql, m)?)?;
    m.add_function(wrap_pyfunction!(get_targets, m)?)?;
    m.add_class::<CompileOptions>()?;
    // From https://github.com/PyO3/maturin/issues/100
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

/// Compilation options for SQL backend of the compiler.
#[pyclass]
#[derive(Clone)]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target dialect to compile to.
    ///
    /// This is only changes the output for a relatively small subset of
    /// features.
    ///
    /// If something does not work in a specific dialect, please raise in a
    /// GitHub issue.
    ///
    /// If `None` is used, the `target` argument from the query header is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub target: String,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

#[pymethods]
impl CompileOptions {
    #[new]
    pub fn new(format: bool, target: String, signature_comment: bool) -> Self {
        CompileOptions {
            format,
            target,
            signature_comment,
        }
    }
}

impl From<CompileOptions> for prql_compiler::Options {
    fn from(o: CompileOptions) -> Self {
        let target = Target::from_str(&o.target).unwrap_or(Target::Sql(Some(Dialect::Generic)));

        prql_compiler::Options {
            format: o.format,
            target,
            signature_comment: o.signature_comment,
        }
    }
}

#[pyfunction]
pub fn get_targets() -> Vec<String> {
    Target::names()
}

#[cfg(not(feature = "extension-module"))]
#[cfg(test)]
mod test {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn parse_for_python() {
        let opts = Some(CompileOptions {
            format: true,
            target: String::new(),
            signature_comment: false,
        });

        assert_snapshot!(
            compile("from employees | filter (age | in 20..30)", opts).unwrap(),
            @r###"
        SELECT
          *
        FROM
          employees
        WHERE
          age BETWEEN 20 AND 30
        "###
        );
    }
}
