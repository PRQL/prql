#![cfg(not(target_family = "wasm"))]
use prql_compiler::{self, sql::Dialect, IntoOnly};
use pyo3::{exceptions, prelude::*};

#[pyfunction]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> PyResult<String> {
    Ok(prql_query)
        .and_then(prql_compiler::pl_of_prql)
        .and_then(prql_compiler::rq_of_pl)
        .and_then(|rq| prql_compiler::sql_of_rq(rq, options.map(prql_compiler::sql::Options::from)))
        .map_err(|e| e.composed("", prql_query, false))
        .map_err(|e| (PyErr::new::<exceptions::PySyntaxError, _>(e.into_only().unwrap().reason)))
}

#[pyfunction]
pub fn pl_of_prql(prql_query: &str) -> PyResult<String> {
    Ok(prql_query)
        .and_then(prql_compiler::pl_of_prql)
        .and_then(prql_compiler::json_of_pl)
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn rq_of_pl(pl_json: &str) -> PyResult<String> {
    Ok(pl_json)
        .and_then(prql_compiler::pl_of_json)
        .and_then(prql_compiler::rq_of_pl)
        .and_then(prql_compiler::json_of_rq)
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn sql_of_rq(rq_json: &str) -> PyResult<String> {
    Ok(rq_json)
        .and_then(prql_compiler::rq_of_json)
        .and_then(|x| prql_compiler::sql_of_rq(x, None))
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pymodule]
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(pl_of_prql, m)?)?;
    m.add_function(wrap_pyfunction!(rq_of_pl, m)?)?;
    m.add_function(wrap_pyfunction!(sql_of_rq, m)?)?;
    // From https://github.com/PyO3/maturin/issues/100
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

/// Compilation options for SQL backend of the compiler.
#[pyclass]
#[derive(Clone)]
pub struct CompileOptions {
    /// True for passing generated SQL string trough a formatter that splits
    /// into multiple lines and prettifies indentation and spacing.
    pub format: bool,

    /// Target dialect you want to compile for.
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic dialect may work with most
    /// databases.
    ///
    /// If something does not work in dialect you need, please report it at
    /// GitHub issues.
    ///
    /// If None is used, `sql_dialect` flag from query definition is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub dialect: Option<Dialect>,
}

impl From<CompileOptions> for prql_compiler::sql::Options {
    fn from(o: CompileOptions) -> Self {
        prql_compiler::sql::Options {
            format: o.format,
            dialect: o.dialect,
        }
    }
}

#[cfg(not(feature = "extension-module"))]
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_for_python() {
        assert_eq!(
            compile("from employees | filter (age | in 20..30)", None).unwrap(),
            "SELECT\n  *\nFROM\n  employees\nWHERE\n  age BETWEEN 20\n  AND 30"
        );
    }
}
