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
join `project-bar.dataset.table` (==col_bax)
```

## Schemas & database names

Identifiers of tables can be prefixed with schema and databases names. Note that
all of following identifiers will be treated as separate table definitions:
`tracks`, `public.tracks`, `my_database.public.tracks`.

```prql
from my_database.chinook.albums
```
