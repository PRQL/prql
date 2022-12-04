# Aggregate

Summarizes many rows into one row.

When applied:

- without `group`, it produces one row from the whole table,
- within a `group` pipeline, it produces one row from each group.

```prql_no_test
aggregate [{expression or assign operations}]
```

```admonish note
Currenty, all declared aggregation functions are `min`, `max`, `count`, `average`, `stddev`, `avg`, `sum` and `count_distinct`. We are in the process of filling out [std lib](../stdlib.html).
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

## Aggregate is required

Unlike in SQL, using an aggregation function in `derive` or `select` (or any other transform except `aggregate`) will not trigger aggreagtion. By default, PRQL will interprete such attempts functions as window functions:

```prql
from employees
derive [avg_sal = average salary]
```

This insures that `derive` does not manipuate number of rows, but only ever adds a column. For more information, see [window transform](./window.html).
