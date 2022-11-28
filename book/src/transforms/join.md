# Join

Adds columns from another table, matching rows based on a condition.

```prql_no_test
join side:{inner|left|right|full} {table} {[conditions]}
```

## Parameters

- `side` decides which rows to include. Defaults to `inner`
- Table reference
- List of conditions
  - Result of join operation is a cartesian (cross) product of rows from both tables,
    which is then filtered to match all of these conditions.

## Examples

```prql
from employees
join side:left positions [id==employee_id]
```

```prql
from employees
join side:left p=positions [id==employee_id]
```

## Self equality operator

If your join conditions are of form `left.x == right.x`,
you can use "self equality operator":

```prql
from employees
join positions [==emp_no]
```
