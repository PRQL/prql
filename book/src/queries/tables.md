# Tables

We can create a table — similar to a CTE in SQL — with `table`:

```prql
table top_50 = (
  from employees
  sort salary
  take 50
  aggregate [total_salary = sum salary]
)

from top_50      # Starts a new pipeline
```

In PRQL `table`s are far less common than CTEs are in SQL, since a linear
series of CTEs can be represented with a single pipeline.

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
