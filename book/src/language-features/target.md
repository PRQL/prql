# Target & Version

## Target dialect

PRQL allows specifying a target dialect at the top of the query, which allows
PRQL to compile to a database-specific SQL flavor.

### Examples

```prql
prql target:sql.postgres

from employees
sort age
take 10
```

```prql
prql target:sql.mssql

from employees
sort age
take 10
```

### Supported dialects

```admonish note
Note that dialect support is early — most differences are not implemented, and
most dialects' implementations are identical to `generic`'s. Contributions are
very welcome.
```

- `sql.ansi`
- `sql.bigquery`
- `sql.clickhouse`
- `sql.generic`
- `sql.hive`
- `sql.mssql`
- `sql.mysql`
- `sql.postgres`
- `sql.sqlite`
- `sql.snowflake`
- `sql.duckdb`

## Version

PRQL allows specifying a version of the language in the PRQL header, like:

```prql
prql version:"0.4"

from employees
```

This has two roles, one of which is implemented:

- The compiler will raise an error if the compiler is older than the query
  version. This prevents confusing errors when queries use newer features of the
  language but the compiler hasn't yet been upgraded.
- The compiler will compile for the major version of the query. This allows the
  language to evolve without breaking existing queries, or forcing multiple
  installations of the compiler. This isn't yet implemented, but is a gating
  feature for PRQL 1.0.
