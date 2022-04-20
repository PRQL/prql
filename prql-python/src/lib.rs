use pyo3::exceptions;
use pyo3::prelude::*;
use prql_compiler::compile;

#[pyfunction]
pub fn to_sql(query: &str) -> PyResult<String> {
    match compile(query) {
        Ok(code) => Ok(code.replace('\n', "")),
        Err(err) => Err(PyErr::new::<exceptions::PyTypeError, _>(format!("{}", err))),
    }
}

#[pymodule]
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_sql, m)?)?;
    Ok(())
}
