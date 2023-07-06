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
fn help() {
    assert_cmd_snapshot!(prqlc_command().arg("--help"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Usage: prqlc [OPTIONS] <COMMAND>

    Commands:
      parse             Parse into PL AST
      fmt               Parse & generate PRQL code back
      debug             Commands for meant for debugging, prone to change
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
fn get_targets() {
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
    sql.mssql
    sql.mysql
    sql.postgres
    sql.sqlite
    sql.snowflake

    ----- stderr -----
    "###);
}

#[test]
fn compile() {
    let mut cmd = StdinCommand::new(get_cargo_bin("prqlc"), "from tracks");
    normalize_prqlc(&mut cmd);
    cmd.args(["compile", "--hide-signature-comment"]);

    assert_cmd_snapshot!(cmd, @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    SELECT
      *
    FROM
      tracks

    ----- stderr -----
    "###);
}

#[test]
fn compile_help() {
  let mut cmd = prqlc_command();
  cmd.args(["compile", "--help"]);

  assert_cmd_snapshot!(cmd, @r###"
  success: true
  exit_code: 0
  ----- stdout -----
  Parse, resolve, lower into RQ & compile to SQL

  Only displays the main pipeline and does not handle loop.

  Usage: prqlc compile [OPTIONS] [INPUT] [OUTPUT] [MAIN_PATH]

  Arguments:
    [INPUT]
            [default: -]

    [OUTPUT]
            [default: -]

    [MAIN_PATH]
            Identifier of the main pipeline

  Options:
        --hide-signature-comment
            With this option set, the output SQL does not have a signature comment at the bottom

    -t, --target <TARGET>
            Target to compile to

            [env: PRQLC_TARGET=]
            [default: sql.any]

        --color <WHEN>
            Controls when to use color

            [default: auto]
            [possible values: auto, always, never]

    -h, --help
            Print help (see a summary with '-h')

  ----- stderr -----
  "###);
}

#[test]
fn compile_project() {
    let mut cmd = prqlc_command();
    cmd.args([
        "compile",
        "--hide-signature-comment",
        project_path().to_str().unwrap(),
        "-",
        "main",
    ]);

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
    cmd.args([
        "compile",
        "--hide-signature-comment",
        project_path().to_str().unwrap(),
        "-",
        "favorite_artists",
    ]);

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
fn format() {
    let mut cmd = StdinCommand::new(get_cargo_bin("prqlc"), "from tracks | take 20");
    normalize_prqlc(&mut cmd);
    cmd.args(["fmt"]);
    assert_cmd_snapshot!(cmd, @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    from tracks
    take 20

    ----- stderr -----
    "###);
}

#[test]
fn shell_completion() {
    for shell in ["bash", "fish", "powershell", "zsh"].iter() {
        assert_cmd_snapshot!(prqlc_command().args(["shell-completion", shell]));
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
    normalize_prqlc(&mut cmd);
    cmd
}

fn normalize_prqlc(cmd: &mut Command) -> &mut Command {
    // We set `CLICOLOR_FORCE` in CI to force color output, but we don't want `prqlc` to
    // output color for our snapshot tests. And it seems to override the
    // `--color=never` flag.
    cmd.env_remove("CLICOLOR_FORCE");
    // We don't want the tests to be affected by the user's `RUST_BACKTRACE` setting.
    cmd.env_remove("RUST_BACKTRACE");
    cmd.env_remove("RUST_LOG");
    cmd.args(["--color=never"]);
    cmd
}
