#![cfg(not(target_family = "wasm"))]

use insta_cmd::get_cargo_bin;
use insta_cmd::{assert_cmd_snapshot, StdinCommand};
use std::env::current_dir;
use std::path::PathBuf;
use std::process::Command;

// Windows has slightly different outputs (e.g. `prqlc.exe` instead of `prqlc`),
// so we exclude.
#[cfg(not(target_family = "windows"))]
#[test]
fn test_help() {
    assert_cmd_snapshot!(prqlc_command().arg("--help"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Usage: prqlc [OPTIONS] <COMMAND>

    Commands:
      parse             Parse into PL AST
      fmt               Parse & generate PRQL code back
      annotate          Parse, resolve & combine source with comments annotating relation type
      debug             Parse & resolve, but don't lower into RQ
      resolve           Parse, resolve & lower into RQ
      sql:preprocess    Parse, resolve, lower into RQ & preprocess SRQ
      sql:anchor        Parse, resolve, lower into RQ & preprocess & anchor SRQ
      compile           Parse, resolve, lower into RQ & compile to SQL
      watch             Watch a directory and compile .prql files to .sql files
      list-targets      Show available compile target names
      shell-completion  Print a shell completion for supported shells
      help              Print this message or the help of the given subcommand(s)

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
    assert_cmd_snapshot!(prqlc_command().arg("list-targets"), @r###"
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
#[ignore = "insta_cmd::StdinCommand is not working correctly"]
fn test_compile() {
    let mut cmd = StdinCommand::new(get_cargo_bin("prqlc"), "from tracks");

    assert_cmd_snapshot!(cmd.arg("compile"), @r###"
    "###);
}

#[test]
fn test_compile_project() {
    let mut cmd = prqlc_command();
    cmd.args(["compile", "--hide-signature-comment"]);
    cmd.arg(project_path());
    cmd.arg("-");
    cmd.arg("main");
    assert_cmd_snapshot!(cmd, @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    WITH table_1 AS (
      SELECT
        120 AS artist_id,
        DATE '2023-05-18' AS last_listen
      UNION
      ALL
      SELECT
        7 AS artist_id,
        DATE '2023-05-16' AS last_listen
    ),
    favorite_artists AS (
      SELECT
        artist_id,
        last_listen
      FROM
        table_1
    ),
    table_0 AS (
      SELECT
        *
      FROM
        read_parquet('artists.parquet')
    ),
    input AS (
      SELECT
        *
      FROM
        table_0
    )
    SELECT
      favorite_artists.artist_id,
      favorite_artists.last_listen,
      input.*
    FROM
      favorite_artists
      LEFT JOIN input ON favorite_artists.artist_id = input.artist_id

    ----- stderr -----
    "###);

    let mut cmd = prqlc_command();
    cmd.args(["compile", "--hide-signature-comment"]);
    cmd.arg(project_path());
    cmd.arg("-");
    cmd.arg("favorite_artists");
    assert_cmd_snapshot!(cmd, @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    WITH table_0 AS (
      SELECT
        120 AS artist_id,
        DATE '2023-05-18' AS last_listen
      UNION
      ALL
      SELECT
        7 AS artist_id,
        DATE '2023-05-16' AS last_listen
    )
    SELECT
      artist_id,
      last_listen
    FROM
      table_0

    ----- stderr -----
    "###);
}

#[test]
fn test_format() {
    let mut cmd = prqlc_command();
    cmd.args(["fmt"]);
    cmd.arg(project_path());
    assert_cmd_snapshot!(cmd, @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    Currently `fmt` only works with a single source, but found multiple sources: "`Project.prql`, `artists.prql`"
    "###);
}

#[test]
fn test_shell_completion() {
    for shell in ["bash", "fish", "powershell", "zsh"].into_iter() {
        assert_cmd_snapshot!(prqlc_command().arg("shell-completion").arg(shell));
    }
}

fn project_path() -> PathBuf {
    current_dir()
        .unwrap()
        // We canonicalize so that it doesn't matter where the cwd is.
        .canonicalize()
        .unwrap()
        .join("tests/project")
}

fn prqlc_command() -> Command {
    let mut cmd = Command::new(get_cargo_bin("prqlc"));
    // We set this in CI to force color output, but we don't want `prqlc` to
    // output color for our snapshot tests. And it seems to override the
    // `--color=never` flag.
    cmd.env_remove("CLICOLOR_FORCE");
    cmd.args(["--color=never"]);
    cmd
}
