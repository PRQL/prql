use arrow::{pyarrow::PyArrowType, record_batch::RecordBatch};
use itertools::Itertools;
use std::str::FromStr;

use lutralib;
use pyo3::prelude::*;

#[pymodule]
fn lutra(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(execute_one, m)?)?;
    // From https://github.com/PyO3/maturin/issues/100
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

#[pyfunction]
pub fn execute_one(
    project_path: &str,
    expression_path: &str,
) -> PyResult<PyArrowType<RecordBatch>> {
    // prepare params
    let discover = lutralib::DiscoverParams {
        project_path: std::path::PathBuf::from_str(project_path)?,
    };
    let execute = lutralib::ExecuteParams {
        expression_path: Some(expression_path.to_string()),
    };

    // run all stages
    let project = lutralib::discover(discover)?;
    let project = lutralib::compile(project, Default::default())?;
    let results = lutralib::execute(project, execute)?;

    // unwrap results
    let (_, relation) = results.into_iter().exactly_one().unwrap();
    let first_record_batch = relation.into_iter().next().unwrap();

    Ok(PyArrowType(first_record_batch))
}
