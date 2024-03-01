#![cfg(not(target_family = "wasm"))]

use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use arrow::record_batch::RecordBatch;
use insta::{assert_debug_snapshot, assert_display_snapshot};
use itertools::Itertools;
use lutra::{DiscoverParams, ExecuteParams};
use prqlc::ir::pl::Ident;

fn example_project_params() -> DiscoverParams {
    DiscoverParams {
        project_path: PathBuf::from_str("../example-project").unwrap(),
    }
}

fn execute_example_project(
    expression_path: Option<String>,
) -> Result<Vec<(Ident, Vec<RecordBatch>)>> {
    let project = lutra::discover(example_project_params())?;

    let project = lutra::compile(project, Default::default())?;

    lutra::execute(project, ExecuteParams { expression_path })
}

#[test]
fn test_discover() {
    let project_tree = lutra::discover(example_project_params()).unwrap();

    let paths: Vec<_> = project_tree.sources.keys().sorted().collect();
    assert_debug_snapshot!(paths, @r###"
    [
        "Project.prql",
        "genres.prql",
    ]
    "###);
}

#[test]
fn test_execute() {
    let results = execute_example_project(Some("main".into())).unwrap();
    let (name, data) = results.into_iter().exactly_one().unwrap();

    assert_eq!(name.to_string(), "main");

    assert_display_snapshot!(arrow::util::pretty::pretty_format_batches(&data).unwrap(), @r###"
    +-----+--------------+-------------+
    | aid | name         | last_listen |
    +-----+--------------+-------------+
    | 240 | Pink Floyd   | 2023-05-18  |
    | 14  | Apocalyptica | 2023-05-16  |
    +-----+--------------+-------------+
    "###);
}

#[test]
fn test_missing() {
    let error = execute_example_project(Some("non_existent".into())).unwrap_err();

    assert_debug_snapshot!(error, @r###"
    Error {
        kind: Error,
        span: None,
        reason: Simple(
            "cannot find expression: `non_existent`",
        ),
        hints: [],
        code: None,
    }
    "###);
}
