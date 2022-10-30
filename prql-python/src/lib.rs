#![cfg(not(target_family = "wasm"))]
use prql_compiler::compile;
use pyo3::exceptions;
use pyo3::prelude::*;

#[pyfunction]
pub fn to_sql(query: &str) -> PyResult<String> {
    compile(query).map_err(|err| PyErr::new::<exceptions::PySyntaxError, _>(err.to_string()))
}

#[pyfunction]
pub fn to_json(query: &str) -> PyResult<String> {
    prql_compiler::to_json(query)
        .map_err(|err| (PyErr::new::<exceptions::PySyntaxError, _>(err.to_string())))
}
#[pymodule]
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_sql, m)?)?;
    m.add_function(wrap_pyfunction!(to_json, m)?)?;
    // From https://github.com/PyO3/maturin/issues/100
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

#[cfg(not(feature = "extension-module"))]
#[cfg(test)]
mod test {
    use super::*;
    use prql_compiler::Result;

    #[test]
    #[ignore]
    fn parse_for_python() -> Result<()> {
        assert_eq!(
            to_sql("from employees | filter (age | in 20..30)")?,
            "SELECT\n  *\nFROM\n  employees\nWHERE\n  age BETWEEN 20\n  AND 30"
        );

        Ok(())
    }
}
