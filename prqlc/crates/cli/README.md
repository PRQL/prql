# PRQL compiler CLI — `prqlc`

`prqlc` serves as a CLI for the PRQL compiler. It is a single, dependency-free
binary that compiles PRQL into SQL.

## Usage

### `prqlc compile`

This command works as a filter that compiles a PRQL string into an SQL string.

```sh
$ echo 'from employees | filter has_dog | select salary' | prqlc compile

SELECT
  salary
FROM
  employees
WHERE
  has_dog
```

A PRQL query can be executed with CLI tools compatible with SQL,, such as
[DuckDB CLI](https://duckdb.org/docs/api/cli.html).

```sh
$ curl -fsL https://raw.githubusercontent.com/PRQL/prql/0.8.1/prql-compiler/tests/integration/data/chinook/albums.csv -o albums.csv
$ echo 'from `albums.csv` | take 3' | prqlc compile | duckdb
┌──────────┬───────────────────────────────────────┬───────────┐
│ album_id │                 title                 │ artist_id │
│  int64   │                varchar                │   int64   │
├──────────┼───────────────────────────────────────┼───────────┤
│        1 │ For Those About To Rock We Salute You │         1 │
│        2 │ Balls to the Wall                     │         2 │
│        3 │ Restless and Wild                     │         2 │
└──────────┴───────────────────────────────────────┴───────────┘
```

Executing this command without any argument will start interactive mode,
allowing a PRQL query to be written interactively. In this mode, after writing
PRQL and press `Ctrl-d` (Linux, macOS) or `Ctrl-z` (Windows) to display the
compiled SQL.

```sh
prqlc compile
```

Just like when using it as a filter, SQL string output can be passed to the
DuckDB CLI and similar tools.

```sh
$ prqlc compile | duckdb
Enter PRQL, then press ctrl-d to compile:

from `albums.csv`
take 3
┌──────────┬───────────────────────────────────────┬───────────┐
│ album_id │                 title                 │ artist_id │
│  int64   │                varchar                │   int64   │
├──────────┼───────────────────────────────────────┼───────────┤
│        1 │ For Those About To Rock We Salute You │         1 │
│        2 │ Balls to the Wall                     │         2 │
│        3 │ Restless and Wild                     │         2 │
└──────────┴───────────────────────────────────────┴───────────┘
```

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/prqlc.svg)](https://repology.org/project/prqlc/versions)

### via Homebrew (macOS, Linux)

```sh
brew install prqlc
```

### via winget (Windows)

```sh
winget install prqlc
```

### From GitHub release page

Precompiled binaries are available for Linux, macOS, and Windows on the
[PRQL release page](https://github.com/PRQL/prql/releases).

### From source

```sh
# From crates.io
cargo install prqlc
```

```sh
# From a local PRQL repository
cargo install --path crates/prqlc
```

### Shell completions

The `prqlc shell-completion` command prints a shell completion script for
supported shells, and saving the printed scripts to files makes for shells to
load completions for each session.

#### Bash

For Linux:

```sh
prqlc shell-completion bash >/etc/bash_completion.d/prqlc
```

For macOS:

```sh
prqlc shell-completion bash >/usr/local/etc/bash_completion.d/prqlc
```

#### fish

```sh
prqlc shell-completion fish >~/.config/fish/completions/prqlc.fish
```

#### PowerShell

```powershell
mkdir -Path (Split-Path -Parent $profile) -ErrorAction SilentlyContinue
prqlc shell-completion powershell >path/to/prqlc.ps1
echo 'Invoke-Expression -Command path/to/prqlc.ps1' >>$profile
```

#### zsh

```sh
prqlc shell-completion zsh >"${fpath[1]}/_prqlc"
```

Ensure that the following lines are present in `~/.zshrc`:

```sh
autoload -U compinit
compinit -i
```

## Helpers

Cheat sheets for `prqlc` are available on various websites and with various
tools.

- [`tldr`](https://tldr.sh/)
  ([on the web](https://tldr.inbrowser.app/pages/common/prqlc))
- [`eg`](https://github.com/srsudar/eg)

<!-- Issues: #2034 cheat/cheatsheets, #2041 devhints.io -->
