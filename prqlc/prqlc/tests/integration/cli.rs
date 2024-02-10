#![cfg(all(not(target_family = "wasm"), feature = "cli"))]

use insta_cmd::assert_cmd_snapshot;
use insta_cmd::get_cargo_bin;
use std::env::current_dir;
use std::path::PathBuf;
use std::process::Command;

#[cfg(not(windows))] // Windows has slightly different output (e.g. `prqlc.exe`), so we exclude.
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
      collect           Parse the whole project and collect it into a single PRQL source file
      debug             Commands for meant for debugging, prone to change
      experimental      Experimental commands are prone to change
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
    sql.glaredb
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
    assert_cmd_snapshot!(prqlc_command()
        .args(["compile", "--hide-signature-comment"])
        .pass_stdin("from tracks"), @r###"
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

#[cfg(not(windows))] // Windows has slightly different output (e.g. `prqlc.exe`), so we exclude.
#[test]
fn compile_help() {
    assert_cmd_snapshot!(prqlc_command().args(["compile", "--help"]), @r###"
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
              Exclude the signature comment containing the PRQL version

          --no-format
              Emit unformatted, dense SQL

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
fn long_query() {
    assert_cmd_snapshot!(prqlc_command()
        .args(["compile", "--hide-signature-comment"])
        .pass_stdin(r#"
let long_query = (
  from employees
  filter gross_cost > 0
  group {title} (
      aggregate {
          ct = count this,
      }
  )
  sort ct
  filter ct > 200
  take 20
  sort ct
  filter ct > 200
  take 20
  sort ct
  filter ct > 200
  take 20
  sort ct
  filter ct > 200
  take 20
)
from long_query
  "#), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    WITH table_2 AS (
      SELECT
        title,
        COUNT(*) AS ct
      FROM
        employees
      WHERE
        gross_cost > 0
      GROUP BY
        title
      HAVING
        COUNT(*) > 200
      ORDER BY
        ct
      LIMIT
        20
    ), table_1 AS (
      SELECT
        title,
        ct
      FROM
        table_2
      WHERE
        ct > 200
      ORDER BY
        ct
      LIMIT
        20
    ), table_0 AS (
      SELECT
        title,
        ct
      FROM
        table_1
      WHERE
        ct > 200
      ORDER BY
        ct
      LIMIT
        20
    ), long_query AS (
      SELECT
        title,
        ct
      FROM
        table_0
      WHERE
        ct > 200
      ORDER BY
        ct
      LIMIT
        20
    )
    SELECT
      title,
      ct
    FROM
      long_query
    ORDER BY
      ct

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

    assert_cmd_snapshot!(prqlc_command()
      .args([
        "compile",
        "--hide-signature-comment",
        project_path().to_str().unwrap(),
        "-",
        "favorite_artists",
    ]), @r###"
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
    // stdin
    assert_cmd_snapshot!(prqlc_command().args(["fmt"]).pass_stdin("from tracks | take 20"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    from tracks
    take 20

    ----- stderr -----
    "###);

    // TODO: not good tests, since they don't actually test that the code was
    // formatted (though we would see the files changed after running the tests
    // if they weren't formatted). Ideally we would have a simulated
    // environment, like a fixture.

    // Single file
    assert_cmd_snapshot!(prqlc_command().args(["fmt", project_path().join("artists.prql").to_str().unwrap()]), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    "###);

    // Project
    assert_cmd_snapshot!(prqlc_command().args(["fmt", project_path().to_str().unwrap()]), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    "###);
}

#[test]
fn debug() {
    assert_cmd_snapshot!(prqlc_command()
        .args(["debug", "resolve"])
        .pass_stdin("from tracks"));

    assert_cmd_snapshot!(prqlc_command()
        .args(["debug", "expand-pl"])
        .pass_stdin("from tracks"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    let main = from tracks

    ----- stderr -----
    "###);

    assert_cmd_snapshot!(prqlc_command()
        .args(["debug", "eval"])
        .pass_stdin("2 + 2"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    # 

    ## main
    4


    ----- stderr -----
    "###);
}

#[test]
fn preprocess() {
    assert_cmd_snapshot!(prqlc_command().args(["sql:preprocess"]).pass_stdin("from tracks | take 20"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    [
        From(
            RIId(
                0,
            ),
        ),
        Super(
            Take(
                Take {
                    range: Range {
                        start: None,
                        end: Some(
                            Expr {
                                kind: Literal(
                                    Integer(
                                        20,
                                    ),
                                ),
                                span: None,
                            },
                        ),
                    },
                    partition: [],
                    sort: [],
                },
            ),
        ),
        Super(
            Select(
                [
                    column-0,
                ],
            ),
        ),
    ]
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
        .join("tests/integration/project")
}

fn prqlc_command() -> Command {
    let mut cmd = Command::new(get_cargo_bin("prqlc"));
    normalize_prqlc(&mut cmd);
    cmd
}

fn normalize_prqlc(cmd: &mut Command) -> &mut Command {
    cmd
        // We set `CLICOLOR_FORCE` in CI to force color output, but we don't want `prqlc` to
        // output color for our snapshot tests. And it seems to override the
        // `--color=never` flag.
        .env_remove("CLICOLOR_FORCE")
        .env("NO_COLOR", "1")
        .args(["--color=never"])
        // We don't want the tests to be affected by the user's `RUST_BACKTRACE` setting.
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LOG")
}
