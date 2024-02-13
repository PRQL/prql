# Target & Version

## Target dialect

PRQL allows specifying a target dialect at the top of the query, which allows
PRQL to compile to a database-specific SQL flavor.

### Examples

```prql
prql target:sql.postgres

from.employees
sort age
take 10
```

```prql
prql target:sql.mssql

from.employees
sort age
take 10
```

## Dialects

### Supported

Supported dialects support all PRQL language features where possible, are tested
on every commit, and we'll endeavor to fix bugs.

- `sql.clickhouse`
- `sql.duckdb`
- `sql.generic`
  {{footnote: while there's no "generic" DB to test `sql.generic` against, we still count it as supported.}}
- `sql.glaredb`
- `sql.mysql`
- `sql.postgres`
- `sql.sqlite`

### Unsupported

Unsupported dialects have implementations in the compiler, but are tested
minimally or not at all, and may have gaps for some features.

We're open to contributions to improve our coverage of these, and to adding
additional dialects.

- `sql.mssql`
- `sql.ansi`
- `sql.bigquery`
- `sql.snowflake`

## Priority of targets

The compile target of a query is defined in the query's header or as an argument
to the compiler. option. The argument to the compiler takes precedence.

For example, the following shell example specifies `sql.generic` in the query
and `sql.duckdb` in the `--target` option of the `prqlc compile` command. In
this case, `sql.duckdb` takes precedence and the SQL output is based on the
DuckDB dialect.

```sh
echo 'prql target:sql.generic
      from.foo' | prqlc compile --target sql.duckdb
```

To use the target described in the query, a special target `sql.any` can be
specified in the compiler option.

```sh
echo 'prql target:sql.generic
      from.foo' | prqlc compile --target sql.any
```

## Version

PRQL allows specifying a version of the language in the PRQL header, like:

```prql
prql version:"0.11.4"

from.employees
```

This has two roles, one of which is implemented:

- The compiler will raise an error if the compiler is older than the query
  version. This prevents confusing errors when queries use newer features of the
  language but the compiler hasn't yet been upgraded.
- The compiler will compile for the major version of the query. This allows the
  language to evolve without breaking existing queries, or forcing multiple
  installations of the compiler. This isn't yet implemented, but is a gating
  feature for PRQL 1.0.

The version of the compiler currently in use can be called using the special
function `std.prql.version` in PRQL.

```prql
[{version = prql.version}]
```

```admonish note
This function was renamed from `std.prql_version` to `prql.version` in PRQL 0.11.1.
`std.prql_version` will be removed in PRQL 0.12.0.
```
