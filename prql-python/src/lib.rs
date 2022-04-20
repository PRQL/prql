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
// This test below is causing a linking error here https://github.com/qorrect/prql/runs/6099160429?check_suite_focus=true
// #[test]
// fn parse_for_python() -> Result<()> {
//     let sql = to_sql("from employees").unwrap();
//     println!("{}", sql);
//     Ok(())
// }
