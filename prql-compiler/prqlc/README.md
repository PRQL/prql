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
# From GitHub
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
$ curl -sL https://raw.githubusercontent.com/mwaskom/seaborn-data/master/penguins.csv -o penguins.csv
$ echo "from `penguins.csv` | take 3" | prqlc compile | duckdb
┌─────────┬───────────┬────────────────┬───────────────┬───────────────────┬─────────────┬─────────┐
│ species │  island   │ bill_length_mm │ bill_depth_mm │ flipper_length_mm │ body_mass_g │   sex   │
│ varchar │  varchar  │     double     │    double     │       int64       │    int64    │ varchar │
├─────────┼───────────┼────────────────┼───────────────┼───────────────────┼─────────────┼─────────┤
│ Adelie  │ Torgersen │           39.1 │          18.7 │               181 │        3750 │ MALE    │
│ Adelie  │ Torgersen │           39.5 │          17.4 │               186 │        3800 │ FEMALE  │
│ Adelie  │ Torgersen │           40.3 │          18.0 │               195 │        3250 │ FEMALE  │
└─────────┴───────────┴────────────────┴───────────────┴───────────────────┴─────────────┴─────────┘
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
from `penguins.csv`
take 3

┌─────────┬───────────┬────────────────┬───────────────┬───────────────────┬─────────────┬─────────┐
│ species │  island   │ bill_length_mm │ bill_depth_mm │ flipper_length_mm │ body_mass_g │   sex   │
│ varchar │  varchar  │     double     │    double     │       int64       │    int64    │ varchar │
├─────────┼───────────┼────────────────┼───────────────┼───────────────────┼─────────────┼─────────┤
│ Adelie  │ Torgersen │           39.1 │          18.7 │               181 │        3750 │ MALE    │
│ Adelie  │ Torgersen │           39.5 │          17.4 │               186 │        3800 │ FEMALE  │
│ Adelie  │ Torgersen │           40.3 │          18.0 │               195 │        3250 │ FEMALE  │
└─────────┴───────────┴────────────────┴───────────────┴───────────────────┴─────────────┴─────────┘
```
