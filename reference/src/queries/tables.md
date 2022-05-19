# Tables

We can create a table — similar to a CTE in SQL — with `table`:

```prql
table top_50 = (
  from employees
  take 50
  aggregate (sum salary)
)

from a            # Starts a new pipeline
```

In PRQL `table`s are far less common that CTEs are in SQL, since a single
pipeline of logic can always be represented without simple pipelines alone.

## Roadmap

Currently it's not yet possible to have an
[s-string](./../language-features/s-strings.md) as a whole table. See
[#376](https://github.com/prql/prql/issues/376) for more details.

<!-- TODO: find an example that we can't currently represent with PRQL -->
```prql_no_test
table a = s"""
  SELECT *
  FROM employees
"""

from a
```
