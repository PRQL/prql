#![cfg(all(not(target_family = "wasm"), feature = "cli"))]

use std::env::current_dir;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use insta_cmd::assert_cmd_snapshot;
use insta_cmd::get_cargo_bin;
use tempfile::TempDir;
use walkdir::WalkDir;

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
      lex               Lex into Lexer Representation
      fmt               Parse & generate PRQL code back
      collect           Parse the whole project and collect it into a single PRQL source file
      debug             Commands for meant for debugging, prone to change
      experimental      Experimental commands are prone to change
      compile           Parse, resolve, lower into RQ & compile to SQL
      watch             Watch a directory and compile .prql files to .sql files
      list-targets      Show available compile target names
      shell-completion  Print a shell completion for supported shells
      help              Print this message or the help of the given subcommand(s)

    Options:
          --color <WHEN>
              Controls when to use color
              
              [default: auto]
              [possible values: auto, always, never]

      -v, --verbose...
              More `v`s, More vebose logging:
              -v shows warnings
              -vv shows info
              -vvv shows debug
              -vvvv shows trace

      -q, --quiet...
              Silences logging output

      -h, --help
              Print help (see a summary with '-h')

      -V, --version
              Print version

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

          --debug-log <DEBUG_LOG>
              File path into which to write the debug log to
              
              [env: PRQLC_DEBUG_LOG=]

          --color <WHEN>
              Controls when to use color
              
              [default: auto]
              [possible values: auto, always, never]

      -v, --verbose...
              More `v`s, More vebose logging:
              -v shows warnings
              -vv shows info
              -vvv shows debug
              -vvvv shows trace

      -q, --quiet...
              Silences logging output

      -h, --help
              Print help (see a summary with '-h')

    ----- stderr -----
    "###);
}

#[test]
fn long_query() {
    assert_cmd_snapshot!(prqlc_command()
        .args(["compile", "--hide-signature-comment", "-vvv", "--debug-log=log_test.html"])
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

    // don't check the contents, they are very prone to change
    assert!(PathBuf::from_str("./log_test.html").unwrap().is_file());
}

#[test]
fn compile_project() {
    let mut cmd = prqlc_command();
    cmd.args([
        "compile",
        "--hide-signature-comment",
        "--debug-log=log_test.json",
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

    // don't check the contents, they are very prone to change
    assert!(PathBuf::from_str("./log_test.json").unwrap().is_file());

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
    // Test stdin formatting
    assert_cmd_snapshot!(prqlc_command().args(["fmt"]).pass_stdin("from tracks | take 20"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    from tracks
    take 20

    ----- stderr -----
    "###);

    // Test formatting a path:

    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Copy files from project_path() to temp_dir
    copy_dir(&project_path(), temp_dir.path());

    // Run fmt command on the temp directory
    let _result = prqlc_command()
        .args(["fmt", temp_dir.path().to_str().unwrap()])
        .status()
        .unwrap();

    // Check if files in temp_dir match the original files
    compare_directories(&project_path(), temp_dir.path());
}

fn copy_dir(src: &Path, dst: &Path) {
    for entry in WalkDir::new(src) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let relative_path = path.strip_prefix(src).unwrap();
            let target_path = dst.join(relative_path);
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::copy(path, target_path).unwrap();
        }
    }
}

fn compare_directories(dir1: &Path, dir2: &Path) {
    for entry in WalkDir::new(dir1).into_iter().filter_map(|e| e.ok()) {
        let path1 = entry.path();
        if path1.is_file() {
            let relative_path = path1.strip_prefix(dir1).unwrap();
            let path2 = dir2.join(relative_path);

            assert!(
                path2.exists(),
                "File {:?} doesn't exist in the formatted directory",
                relative_path
            );

            similar_asserts::assert_eq!(
                fs::read_to_string(path1).unwrap(),
                fs::read_to_string(path2).unwrap()
            );
        }
    }
}

#[test]
fn debug() {
    assert_cmd_snapshot!(prqlc_command()
        .args(["debug", "lineage"])
        .pass_stdin("from tracks | select {artist, album}"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    frames:
    - - 1:14-36
      - columns:
        - !Single
          name:
          - tracks
          - artist
          target_id: 120
          target_name: null
        - !Single
          name:
          - tracks
          - album
          target_id: 121
          target_name: null
        inputs:
        - id: 118
          name: tracks
          table:
          - default_db
          - tracks
    nodes:
    - id: 118
      kind: Ident
      span: 1:0-11
      ident: !Ident
      - default_db
      - tracks
      parent: 123
    - id: 120
      kind: Ident
      span: 1:22-28
      ident: !Ident
      - this
      - tracks
      - artist
      targets:
      - 118
      parent: 122
    - id: 121
      kind: Ident
      span: 1:30-35
      ident: !Ident
      - this
      - tracks
      - album
      targets:
      - 118
      parent: 122
    - id: 122
      kind: Tuple
      span: 1:21-36
      children:
      - 120
      - 121
      parent: 123
    - id: 123
      kind: 'TransformCall: Select'
      span: 1:14-36
      children:
      - 118
      - 122
    ast:
      name: Project
      stmts:
      - VarDef:
          kind: Main
          name: main
          value:
            Pipeline:
              exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: 1:0-4
                  args:
                  - Ident: tracks
                    span: 1:5-11
                span: 1:0-11
              - FuncCall:
                  name:
                    Ident: select
                    span: 1:14-20
                  args:
                  - Tuple:
                    - Ident: artist
                      span: 1:22-28
                    - Ident: album
                      span: 1:30-35
                    span: 1:21-36
                span: 1:14-36
            span: 1:0-36
        span: 1:0-36

    ----- stderr -----
    "###);

    // Don't test the output of this, since on one min-versions check it had
    // different results, and didn't repro on Mac. It having different results
    // makes it difficult to debug, and we get most of the value by just
    // checking it runs successfully.
    prqlc_command()
        .args(["debug", "ast"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
}

// The output of `prqlc debug json-schema` is long, so rather than
// comparing the full output as a snapshot, we just verify that the
// standard output parses as JSON and check a couple top-level keys.
#[test]
fn debug_json_schema() {
    use serde_json::Value;

    let output = prqlc_command()
        .args(["debug", "json-schema", "--ir-type", "pl"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let parsed: Value = serde_json::from_str(stdout).unwrap();

    assert_eq!(
        parsed["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(parsed["type"], "object");
    assert_eq!(parsed["title"], "ModuleDef");
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

#[test]
fn compile_no_prql_files() {
    assert_cmd_snapshot!(prqlc_command().args(["compile", "README.md"]), @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    Error: No `.prql` files found in the source tree

    "###);
}

#[test]
fn lex() {
    assert_cmd_snapshot!(prqlc_command().args(["lex"]).pass_stdin("from tracks"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    - kind: Start
      span:
        start: 0
        end: 0
    - kind: !Ident from
      span:
        start: 0
        end: 4
    - kind: !Ident tracks
      span:
        start: 5
        end: 11

    ----- stderr -----
    "###);

    assert_cmd_snapshot!(prqlc_command().args(["lex", "--format=json"]).pass_stdin("from tracks"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    [
      {
        "kind": "Start",
        "span": {
          "start": 0,
          "end": 0
        }
      },
      {
        "kind": {
          "Ident": "from"
        },
        "span": {
          "start": 0,
          "end": 4
        }
      },
      {
        "kind": {
          "Ident": "tracks"
        },
        "span": {
          "start": 5,
          "end": 11
        }
      }
    ]
    ----- stderr -----
    "###);
}
