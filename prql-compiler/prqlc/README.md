# prqlc

## Installation

`prqlc` is a single, dependency-free binary that compiles PRQL into SQL.
precompiled binaries are available for Linux, macOS, and Windows on the
[PRQL release page](https://github.com/PRQL/prql/releases).

`prqlc` can be installed via `cargo`:

```sh
# From crates.io
cargo install prqlc
```

```sh
# From a local PRQL repository
cargo install --path prql-compiler/prqlc
```

<!-- It can be installed via brew too:

```sh
brew install prql/prql/prql-compiler
``` -->

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
$ prqlc compile
```

As with using it as a filter, you can pass the SQL string output to the DuckDB
CLI, etc.

```sh
$ prqlc compile | duckdb
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
