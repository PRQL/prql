#![cfg(not(target_family = "wasm"))]
use std::str::FromStr;

use prqlc_lib::ErrorMessages;
use pyo3::{exceptions, prelude::*};

#[pyfunction]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> PyResult<String> {
    let Ok(options) = options.map(convert_options).transpose() else {
        return Err(PyErr::new::<exceptions::PyValueError, _>(
            "Invalid options".to_string(),
        ));
    };

    Ok(prql_query)
        .and_then(prqlc_lib::prql_to_pl)
        .and_then(prqlc_lib::pl_to_rq)
        .and_then(|rq| prqlc_lib::rq_to_sql(rq, &options.unwrap_or_default()))
        .map_err(|e| e.composed(&prql_query.into()))
        .map_err(|e| (PyErr::new::<exceptions::PyValueError, _>(e.to_string())))
}

#[pyfunction]
pub fn prql_to_pl(prql_query: &str) -> PyResult<String> {
    prqlc_lib::prql_to_pl(prql_query)
        .and_then(|x| prqlc_lib::json::from_pl(&x))
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn pl_to_rq(pl_json: &str) -> PyResult<String> {
    prqlc_lib::json::to_pl(pl_json)
        .and_then(prqlc_lib::pl_to_rq)
        .and_then(|x| prqlc_lib::json::from_rq(&x))
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pyfunction]
pub fn rq_to_sql(rq_json: &str, options: Option<CompileOptions>) -> PyResult<String> {
    prqlc_lib::json::to_rq(rq_json)
        .and_then(|x| {
            prqlc_lib::rq_to_sql(
                x,
                &options
                    .map(convert_options)
                    .transpose()?
                    .unwrap_or_default(),
            )
        })
        .map_err(|err| (PyErr::new::<exceptions::PyValueError, _>(err.to_json())))
}

#[pymodule]
fn prqlc(_py: Python, m: &PyModule) -> PyResult<()> {
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
#[derive(Clone, Debug)]
pub struct CompileOptions {
    /// Pass generated SQL string through a formatter that splits it into
    /// multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target to compile to.
    ///
    /// Defaults to "sql.any", which uses the `target` argument from the query
    /// header to determine The SQL dialect.
    pub target: String,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,

    pub color: bool,

    pub display: String,
}

#[pymethods]
impl CompileOptions {
    #[new]
    #[pyo3(signature = (*, format=true, signature_comment=true, target="sql.any".to_string(), color=false, display="plain".to_string()))]
    pub fn new(
        format: bool,
        signature_comment: bool,
        target: String,
        color: bool,
        display: String,
    ) -> Self {
        CompileOptions {
            format,
            target,
            signature_comment,
            color,
            display: display.to_lowercase(),
        }
    }
}

fn convert_options(o: CompileOptions) -> Result<prqlc_lib::Options, prqlc_lib::ErrorMessages> {
    use prqlc_lib::Error;
    let target = prqlc_lib::Target::from_str(&o.target).map_err(prqlc_lib::ErrorMessages::from)?;

    Ok(prqlc_lib::Options {
        format: o.format,
        target,
        signature_comment: o.signature_comment,
        color: false,
        display: prqlc_lib::DisplayOptions::from_str(&o.display).map_err(|e| ErrorMessages {
            inner: vec![Error::new_simple(format!("Invalid display option: {}", e)).into()],
        })?,
    })
}

#[pyfunction]
pub fn get_targets() -> Vec<String> {
    prqlc_lib::Target::names()
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
            target: "sql.any".to_string(),
            signature_comment: false,
            color: false,
            display: "plain".to_string(),
        });

        assert_snapshot!(
            compile("from db.employees | filter (age | in 20..30)", opts).unwrap(),
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

    #[test]
    fn parse_pipeline() {
        let opts = Some(CompileOptions {
            format: true,
            target: "sql.any".to_string(),
            signature_comment: false,
            color: false,
            display: "plain".to_string(),
        });

        let prql = r#"from db.artists | select {name, id} | filter (id | in [1, 2, 3])"#;
        assert_snapshot!(
             prql_to_pl(prql).and_then(|x| pl_to_rq(x.as_str())).and_then(|x|rq_to_sql(x.as_str(), opts)).unwrap(), @r###"
        SELECT
          name,
          id
        FROM
          artists
        WHERE
          id IN (1, 2, 3)
        "###);
    }
}
