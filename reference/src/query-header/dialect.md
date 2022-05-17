# Dialect

PRQL allows specifying a dialect at the top of the query, which allows PRQL to
compile to a database-specific SQL flavor.

## Examples

```prql
prql dialect:postgres

from employees
sort age
take 10
```

```prql
prql dialect:mssql

from employees
sort age
take 10
```

## Supported dialects

> Note that dialect support is _very_ early — most differences are not
> implemented, and most dialects have no difference in implementation from
> `generic` at all all. Contributions are very welcome.

- `ansi`
- `bigquery`
- `clickhouse`
- `generic`
- `hive`
- `mssql`
- `mysql`
- `postgres`
- `sqlite`
- `snowflake`
