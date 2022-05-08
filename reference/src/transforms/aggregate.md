## Aggregate

> group rows by one or more columns

```prql_no_test
aggregate [{expression or assign operations}]
```

#### Examples

```prql
from employees
aggregate [
  average salary,
  ct = count
]
```
