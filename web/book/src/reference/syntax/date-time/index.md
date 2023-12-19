# Date and time

We already know that PRQL uses the `@2020-01-01` syntax to declare dates. To
manipulate those dates, PRQL has a `date` module with some useful functions

### `to_text`

This function allows to convert a date into a text. Since there are many
possible date representations, `to_text` takes a `format` parameter that
describes thanks to [specifiers](./format-specifiers.md) how the date or
timestamp should be structured.

```admonish info
Since all RDBMS have different ways to format dates and times, PRQL **requires an explicit dialect** to be specified
```

```admonish info
For now the supported DBs are: Clickhouse, DuckDB, MySQL, MSSQL and Postgres.
```

```prql
prql target:sql.duckdb

from invoices
select {
  invoice_date | date.to_text "%d/%m/%Y"
}
```

```prql
prql target:sql.postgres

from invoices
select {
  invoice_date | date.to_text "%d/%m/%Y"
}
```

```prql
prql target:sql.mysql

from invoices
select {
  invoice_date | date.to_text "%d/%m/%Y"
}
```
