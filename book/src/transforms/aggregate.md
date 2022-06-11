# Aggregate

Summarizes many rows into one row.

When applied:

- without `group`, it produces one row from the whole table,
- within a `group` pipeline, it produces one row from each group.

```prql_no_test
aggregate [{expression or assign operations}]
```

## Examples

```prql
from employees
aggregate [
  average salary,
  ct = count
]
```

```prql
from employees
group [title, country] (
  aggregate [
    average salary,
    ct = count
  ]
)
```
