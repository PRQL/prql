# PRQL compiler CLI — `prqlc`

`prqlc` is a CLI for PRQL compiler; A single, dependency-free binary that
compiles PRQL into SQL.

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/prqlc.svg)](https://repology.org/project/prqlc/versions)

### via Homebrew (macOS, Linux)

```sh
brew install prqlc
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
cargo install --path prql-compiler/prqlc
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

## Usage

### prqlc compile

This command is working as a filter to compile PRQL string into SQL string.

```sh
$ echo "from employees | filter has_dog | select salary" | prqlc compile

SELECT
  salary
FROM
  employees
WHERE
  has_dog
```

PRQL query can be executed with CLI tools working with SQL, such as
[DuckDB CLI](https://duckdb.org/docs/api/cli.html).

```sh
$ curl -sL https://raw.githubusercontent.com/PRQL/prql/0.8.1/prql-compiler/tests/integration/data/chinook/albums.csv -o albums.csv
$ echo "from `albums.csv` | take 3" | prqlc compile | duckdb
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
allowing you to write PRQL query interactively. In this mode, after you write
PRQL and press `Ctrl-D` (Linux, macOS) or `Ctrl-Z` (Windows) to display the
compiled SQL.

```sh
prqlc compile
```

As with using it as a filter, you can pass the SQL string output to the DuckDB
CLI, etc.

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
