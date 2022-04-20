use pyo3::exceptions;
use pyo3::prelude::*;
use prql_compiler::compile;

#[pyfunction]
fn compile_prql(query: &str) -> PyResult<String> {

    match compile(query) {
        Ok(code) => Ok(code),
        Err(err) => Err(PyErr::new::<exceptions::PyTypeError, _>(format!("{}", err))),
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile_prql, m)?)?;
    Ok(())
}
