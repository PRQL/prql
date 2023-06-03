# Distinct

PRQL doesn't have a specific `distinct` keyword. Instead, use `group` and
`take 1`:

```prql
from employees
select department
group department (
  take 1
)
```

This also works without a linebreak:

```prql
from employees
select department
group department (take 1)
```

## Selecting from each group

We are be able to
[select a single row from each group](https://stackoverflow.com/questions/3800551/select-first-row-in-each-group-by-group)
by combining `group` and `sort`:

```prql
# youngest employee from each department
from employees
group department (
  sort age
  take 1
)
```

Note that we can't always compile to `DISTINCT`; when the columns in the `group`
aren't all the available columns, we need to use a window function:

```prql
from employees
group {first_name, last_name} (take 1)
```

## Distinct on

When compiling to Postgres or DuckDB dialect, `DISTINCT ON` is used to take one
row for each group:

```prql
prql target:sql.postgres

from employees
group department (
  sort age
  take 1
)
```
