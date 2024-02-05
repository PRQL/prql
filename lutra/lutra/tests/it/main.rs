#![cfg(not(target_family = "wasm"))]

use std::{path::PathBuf, str::FromStr};

use insta::{assert_debug_snapshot, assert_display_snapshot};
use itertools::Itertools;
use lutra::{DiscoverParams, ExecuteParams};

fn example_project_params() -> DiscoverParams {
    DiscoverParams {
        project_path: PathBuf::from_str("../example-project").unwrap(),
    }
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
    let params = ExecuteParams {
        discover: example_project_params(),
        expression_path: Some("main".to_string()),
    };

    let results = lutra::execute(params).unwrap();
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
    let params = ExecuteParams {
        discover: example_project_params(),
        expression_path: Some("non_existent".to_string()),
    };

    assert_debug_snapshot!(lutra::execute(params).unwrap_err(), @r###"
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
