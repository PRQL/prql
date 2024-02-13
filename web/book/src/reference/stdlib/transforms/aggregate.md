# Aggregate

Summarizes many rows into one row.

When applied:

- without `group`, it produces one row from the whole table,
- within a `group` pipeline, it produces one row from each group.

```prql no-eval
aggregate {expression or assign operations}
```

```admonish note
Currently, all declared aggregation functions are `min`, `max`, `count`,
`average`, `stddev`, `avg`, `sum` and `count_distinct`. We are in the
process of filling out [std lib](../).
```

## Examples

```prql
from.employees
aggregate {
  average salary,
  ct = count salary
}
```

```prql
from.employees
group {title, country} (
  aggregate {
    average salary,
    ct = count salary,
  }
)
```

## Aggregate is required

Unlike in SQL, using an aggregation function in `derive` or `select` (or any
other transform except `aggregate`) will not trigger aggregation. By default,
PRQL will interpret such attempts functions as window functions:

```prql
from.employees
derive {avg_sal = average salary}
```

This ensures that `derive` does not manipulate the number of rows, but only ever
adds a column. For more information, see [window transform](./window.md).
