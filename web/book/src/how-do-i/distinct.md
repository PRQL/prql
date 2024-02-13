# How do I: remove duplicates?

PRQL doesn't have a specific `distinct` keyword. Instead duplicate tuples in a
relation can be removed by using `group` and `take 1`:

```prql
from.employees
select department
group employees.* (
  take 1
)
```

This also works with a wildcard:

```prql
from.employees
group employees.* (take 1)
```

## Remove duplicates from each group?

To
[select a single row from each group](https://stackoverflow.com/questions/3800551/select-first-row-in-each-group-by-group)
`group` can be combined with `sort` and `take`:

```prql
# youngest employee from each department
from.employees
group department (
  sort age
  take 1
)
```

Note that we can't always compile to `DISTINCT`; when the columns in the `group`
aren't all the available columns, we need to use a window function:

```prql
from.employees
group {first_name, last_name} (take 1)
```

<!-- TODO: uncomment when the bug is fixed -->

<!-- When compiling to Postgres or DuckDB dialect, such queries will be compiled to
`DISTINCT ON`, which is
[the most performant option](https://stackoverflow.com/a/7630564).

```prql
prql target:sql.postgres

from.employees
group department (
  sort age
  take 1
)
``` -->
