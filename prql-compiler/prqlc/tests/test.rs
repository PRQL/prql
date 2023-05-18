#![cfg(not(target_family = "wasm"))]

use insta_cmd::get_cargo_bin;
use insta_cmd::{assert_cmd_snapshot, StdinCommand};
use std::process::Command;

// Windows has slightly different outputs (e.g. `prqlc.exe` instead of `prqlc`),
// so we exclude.
#[cfg(not(target_family = "windows"))]
#[test]
fn test_help() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("--help"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Usage: prqlc [OPTIONS] <COMMAND>

    Commands:
      parse           Parse into PL AST
      fmt             Parse & generate PRQL code back
      annotate        Parse, resolve & combine source with comments annotating relation type
      debug           Parse & resolve, but don't lower into RQ
      resolve         Parse, resolve & lower into RQ
      sql:preprocess  Parse, resolve, lower into RQ & preprocess SRQ
      sql:anchor      Parse, resolve, lower into RQ & preprocess & anchor SRQ
      compile         Parse, resolve, lower into RQ & compile to SQL
      watch           Watch a directory and compile .prql files to .sql files
      list-targets    Show available compile target names
      help            Print this message or the help of the given subcommand(s)

    Options:
          --color <WHEN>  Controls when to use color [default: auto] [possible values: auto, always,
                          never]
      -h, --help          Print help
      -V, --version       Print version

    ----- stderr -----
    "###);
}

#[test]
fn test_get_targets() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("list-targets"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    sql.any
    sql.ansi
    sql.bigquery
    sql.clickhouse
    sql.duckdb
    sql.generic
    sql.hive
    sql.mssql
    sql.mysql
    sql.postgres
    sql.sqlite
    sql.snowflake

    ----- stderr -----
    "###);
}

#[test]
fn test_compile() {
    let mut cmd = StdinCommand::new(get_cargo_bin("prqlc"), "from tracks");

    // TODO: fix
    assert_cmd_snapshot!(cmd.arg("compile"), @r###"
    success: false
    exit_code: 1
    ----- stdout -----
    [E0001] Error: Missing main pipeline

    ----- stderr -----
    "###);
}
