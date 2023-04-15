# Quoting identifiers

To use identifiers that are otherwise invalid, surround them with backticks.
Depending on the dialect, these will remain as backticks or be converted to
double-quotes.

```prql
prql target:sql.mysql
from employees
select `first name`
```

```prql
prql target:sql.postgres
from employees
select `first name`
```

```prql
from `dir/*.parquet`
```

BigQuery also uses backticks to surround project & dataset names (even if valid
identifiers) in the `SELECT` statement:

```prql
prql target:sql.bigquery
from `project-foo.dataset.table`
join `project-bar.dataset.table` [==col_bax]
```

## Quoting schemas

```admonish note
This is currently not great and we are working on improving it; see
https://github.com/PRQL/prql/issues/1535 for progress.
```

If supplying a schema without a column — for example in a `from` or `join`
transform, that also needs to be a quoted identifier:

```prql
from `music.albums`
```
