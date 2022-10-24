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
    which is the filtered to match all of these conditions.
  - If all terms are only column identifiers,
    columns with these names from both tables with be tested for equality to one another.
    For example, `[col1, col2]` is equivalent to `[left.col1 == right.col1, left.col2 == right.col2]`

## Examples

```prql
from employees
join side:left positions [id==employee_id]
```

```prql
from employees
join side:full positions [emp_no]
```

```prql
from employees
join side:left p=positions [id==employee_id]
```
