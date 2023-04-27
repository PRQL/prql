#![cfg(not(target_family = "wasm"))]

use prql_compiler::Target;

#[test]
fn get_targets() {
    use assert_cmd;
    let n_targets = Target::names().len();

    assert_cmd::Command::cargo_bin("prqlc")
        .unwrap()
        .args(["get-targets"])
        .assert()
        .success()
        .stdout(
            predicates::str::is_match(r"sql\.[a-z]+\n")
                .unwrap()
                .count(n_targets),
        );
}
