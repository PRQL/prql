#![cfg(not(target_family = "wasm"))]
use std::str::FromStr;

use prqlc_lib::ErrorMessages;
use pyo3::{exceptions, prelude::*};

#[pyfunction]
#[pyo3(signature = (prql_query, options=None))]
pub fn compile(prql_query: &str, options: Option<CompileOptions>) -> PyResult<String> {
    let Ok(options) = options.map(convert_options).transpose() else {
        return Err(PyErr::new::<exceptions::PyValueError, _>(
            "Invalid options".to_string(),
        ));
    };

    prqlc_lib::compile(prql_query, &options.unwrap_or_default())
        .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_string()))
}

#[pyfunction]
pub fn prql_to_pl(prql_query: &str) -> PyResult<String> {
    prqlc_lib::prql_to_pl(prql_query)
        .and_then(|x| prqlc_lib::json::from_pl(&x))
        .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
}

#[pyfunction]
pub fn pl_to_prql(pl_json: &str) -> PyResult<String> {
    prqlc_lib::json::to_pl(pl_json)
        .and_then(|x| prqlc_lib::pl_to_prql(&x))
        .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
}

#[pyfunction]
pub fn pl_to_rq(pl_json: &str) -> PyResult<String> {
    prqlc_lib::json::to_pl(pl_json)
        .and_then(prqlc_lib::pl_to_rq)
        .and_then(|x| prqlc_lib::json::from_rq(&x))
        .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
}

#[pyfunction]
#[pyo3(signature = (rq_json, options=None))]
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
        .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
}

mod debug {
    use super::*;

    #[pyfunction]
    pub fn prql_lineage(prql_query: &str) -> PyResult<String> {
        prqlc_lib::prql_to_pl(prql_query)
            .and_then(prqlc_lib::internal::pl_to_lineage)
            .and_then(|x| prqlc_lib::internal::json::from_lineage(&x))
            .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
    }

    #[pyfunction]
    pub fn pl_to_lineage(pl_json: &str) -> PyResult<String> {
        prqlc_lib::json::to_pl(pl_json)
            .and_then(prqlc_lib::internal::pl_to_lineage)
            .and_then(|x| prqlc_lib::internal::json::from_lineage(&x))
            .map_err(|err| PyErr::new::<exceptions::PyValueError, _>(err.to_json()))
    }
}

#[pymodule]
fn prqlc(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(prql_to_pl, m)?)?;
    m.add_function(wrap_pyfunction!(pl_to_prql, m)?)?;
    m.add_function(wrap_pyfunction!(pl_to_rq, m)?)?;
    m.add_function(wrap_pyfunction!(rq_to_sql, m)?)?;
    m.add_function(wrap_pyfunction!(get_targets, m)?)?;

    m.add_class::<CompileOptions>()?;
    // From https://github.com/PyO3/maturin/issues/100
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // add debug submodule
    let debug_module = PyModule::new(_py, "debug")?;
    debug_module.add_function(wrap_pyfunction!(debug::prql_lineage, &debug_module)?)?;
    debug_module.add_function(wrap_pyfunction!(debug::pl_to_lineage, &debug_module)?)?;

    m.add_submodule(&debug_module)?;

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
            inner: vec![Error::new_simple(format!("Invalid display option: {e}")).into()],
        })?,
    })
}

#[pyfunction]
pub fn get_targets() -> Vec<String> {
    prqlc_lib::Target::names()
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::*;

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
            compile("from employees | filter (age | in 20..30)", opts).unwrap(),
            @"
        SELECT
          *
        FROM
          employees
        WHERE
          age BETWEEN 20 AND 30
        "
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

        let prql = r#"from artists | select {name, id} | filter (id | in [1, 2, 3])"#;
        assert_snapshot!(
             prql_to_pl(prql).and_then(|x| pl_to_rq(x.as_str())).and_then(|x|rq_to_sql(x.as_str(), opts)).unwrap(), @"
        SELECT
          name,
          id
        FROM
          artists
        WHERE
          id IN (1, 2, 3)
        ");
    }

    #[test]
    fn prql_pl_prql_roundtrip() {
        let prql = r#"from artists | select {name, id} | filter (id | in [1, 2, 3])"#;
        assert_snapshot!(
             prql_to_pl(prql).and_then(|x| pl_to_prql(x.as_str())).unwrap(), @"
        from artists
        select {name, id}
        filter (id | in [1, 2, 3])
        ");
    }

    #[test]
    fn debug_prql_lineage() {
        assert_snapshot!(
            debug::prql_lineage(r#"from a | select { beta, gamma }"#).unwrap(),
            @r#"{"frames":[["1:9-31",{"columns":[{"Single":{"name":["a","beta"],"target_id":8,"target_name":null}},{"Single":{"name":["a","gamma"],"target_id":9,"target_name":null}}],"inputs":[{"id":7,"name":"a","table":["default_db","a"]}]}]],"nodes":[{"id":7,"kind":"Ident","span":"1:0-6","ident":{"Ident":["default_db","a"]},"parent":11},{"id":8,"kind":"Ident","span":"1:18-22","ident":{"Ident":["this","a","beta"]},"targets":[7],"parent":10},{"id":9,"kind":"Ident","span":"1:24-29","ident":{"Ident":["this","a","gamma"]},"targets":[7],"parent":10},{"id":10,"kind":"Tuple","span":"1:16-31","children":[8,9],"parent":11},{"id":11,"kind":"TransformCall: Select","span":"1:9-31","children":[7,10]}],"ast":{"name":"Project","stmts":[{"VarDef":{"kind":"Main","name":"main","value":{"Pipeline":{"exprs":[{"FuncCall":{"name":{"Ident":["from"],"span":"1:0-4"},"args":[{"Ident":["a"],"span":"1:5-6"}]},"span":"1:0-6"},{"FuncCall":{"name":{"Ident":["select"],"span":"1:9-15"},"args":[{"Tuple":[{"Ident":["beta"],"span":"1:18-22"},{"Ident":["gamma"],"span":"1:24-29"}],"span":"1:16-31"}]},"span":"1:9-31"}]},"span":"1:0-31"}},"span":"1:0-31"}]}}"#
        );
    }

    #[test]
    fn debug_pl_to_lineage() {
        assert_snapshot!(
            prql_to_pl(r#"from a | select { beta, gamma }"#).and_then(|x| debug::pl_to_lineage(&x)).unwrap(),
            @r#"{"frames":[["1:9-31",{"columns":[{"Single":{"name":["a","beta"],"target_id":8,"target_name":null}},{"Single":{"name":["a","gamma"],"target_id":9,"target_name":null}}],"inputs":[{"id":7,"name":"a","table":["default_db","a"]}]}]],"nodes":[{"id":7,"kind":"Ident","span":"1:0-6","ident":{"Ident":["default_db","a"]},"parent":11},{"id":8,"kind":"Ident","span":"1:18-22","ident":{"Ident":["this","a","beta"]},"targets":[7],"parent":10},{"id":9,"kind":"Ident","span":"1:24-29","ident":{"Ident":["this","a","gamma"]},"targets":[7],"parent":10},{"id":10,"kind":"Tuple","span":"1:16-31","children":[8,9],"parent":11},{"id":11,"kind":"TransformCall: Select","span":"1:9-31","children":[7,10]}],"ast":{"name":"Project","stmts":[{"VarDef":{"kind":"Main","name":"main","value":{"Pipeline":{"exprs":[{"FuncCall":{"name":{"Ident":["from"],"span":"1:0-4"},"args":[{"Ident":["a"],"span":"1:5-6"}]},"span":"1:0-6"},{"FuncCall":{"name":{"Ident":["select"],"span":"1:9-15"},"args":[{"Tuple":[{"Ident":["beta"],"span":"1:18-22"},{"Ident":["gamma"],"span":"1:24-29"}],"span":"1:16-31"}]},"span":"1:9-31"}]},"span":"1:0-31"}},"span":"1:0-31"}]}}"#
        );
    }
}

/// `pyo3-build-config` is PyO3's own build-script helper, released in lockstep
/// with `pyo3`, and PyO3 expects the two to be on the same version. Dependabot
/// tracks them as independent entries, so merging one bump without the other
/// silently splits them: #6258 took `pyo3-build-config` to 0.29.2 while `pyo3`
/// stayed on 0.27, leaving two copies of the crate in the lockfile and building
/// this crate's extension-module link args with a helper two minor versions
/// ahead of the `pyo3` it links against.
///
/// The `pyo3` group in `.github/dependabot.yaml` keeps the bumps together; this
/// catches a split that arrives some other way. The requirements in
/// `[workspace.dependencies]` are what Dependabot edits, so they're checked
/// directly rather than the resolved versions in `Cargo.lock`.
#[cfg(test)]
mod pyo3_versions {
    use insta::assert_snapshot;

    #[test]
    fn are_aligned() {
        let manifest = include_str!("../../../../Cargo.toml");
        let pyo3 = workspace_dep_version(manifest, "pyo3").expect("`pyo3` is a workspace dep");
        let build_config = workspace_dep_version(manifest, "pyo3-build-config")
            .expect("`pyo3-build-config` is a workspace dep");

        assert_eq!(
            minor_version(&pyo3),
            minor_version(&build_config),
            "`pyo3` ({pyo3}) and `pyo3-build-config` ({build_config}) must be on the same minor \
             version — bump both together in the workspace `Cargo.toml`"
        );
    }

    /// The version requirement `name` is pinned to in `[workspace.dependencies]`,
    /// in either the shorthand (`foo = "1.2.3"`) or table (`foo = { version =
    /// "1.2.3", … }`) form, the latter possibly wrapped across lines.
    fn workspace_dep_version(manifest: &str, name: &str) -> Option<String> {
        let deps = manifest.split_once("\n[workspace.dependencies]\n")?.1;
        // Section headers are the only thing starting a line with `[` here.
        let deps = deps.split_once("\n[").map_or(deps, |(section, _)| section);

        let entry = deps.lines().position(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| key.trim() == name)
        })?;
        let entry = deps.lines().skip(entry).collect::<Vec<_>>().join("\n");
        let value = entry.split_once('=')?.1.trim_start();

        match value.strip_prefix('{') {
            None => leading_string(value),
            Some(table) => {
                let table = table.split_once('}')?.0;
                let version = table.split_once("version")?.1.trim_start();
                leading_string(version.strip_prefix('=')?.trim_start())
            }
        }
    }

    /// The contents of the `"…"` string `s` starts with.
    fn leading_string(s: &str) -> Option<String> {
        s.strip_prefix('"')?
            .split_once('"')
            .map(|(value, _)| value.to_string())
    }

    /// `major.minor` of a version — PyO3 pairs the two crates per minor release,
    /// so a differing patch level between them is not a mismatch.
    fn minor_version(version: &str) -> &str {
        match version.match_indices('.').nth(1) {
            Some((end, _)) => &version[..end],
            None => version,
        }
    }

    #[test]
    fn parses_dep_versions() {
        let manifest = r#"
[workspace.package]
version = "0.13.15"

[workspace.dependencies]
anyhow = "1.0.103"
arrow = { version = "53", features = [
  "pyarrow",
], default-features = false }
pyo3-build-config = "0.27.1"
pyo3 = { version = "0.27.1", features = ["abi3-py37", "anyhow"] }

[profile.release]
lto = true
"#;

        let found = [
            "pyo3",
            "pyo3-build-config",
            "arrow",
            "anyhow",
            "lto",
            "serde",
        ]
        .map(|name| format!("{name}: {:?}", workspace_dep_version(manifest, name)))
        .join("\n");

        assert_snapshot!(found, @r#"
        pyo3: Some("0.27.1")
        pyo3-build-config: Some("0.27.1")
        arrow: Some("53")
        anyhow: Some("1.0.103")
        lto: None
        serde: None
        "#);
    }

    #[test]
    fn truncates_to_minor_version() {
        let found = ["0.27.1", "0.27", "1.0.103", "53"]
            .map(|version| minor_version(version).to_string())
            .join(" ");

        assert_snapshot!(found, @"0.27 0.27 1.0 53");
    }
}
