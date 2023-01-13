# Join

Adds columns from another table, matching rows based on a condition.

```prql_no_test
join side:{inner|left|right|full} {table} {[conditions]}
```

## Parameters

- `side` decides which rows to include, defaulting to `inner`.
- Table reference
- List of conditions
  - The result of join operation is a cartesian (cross) product of rows from
    both tables, which is then filtered to match all of these conditions.
  - If name is the same from both tables, it can be expressed with only `==col`.

## Examples

```prql
from employees
join side:left positions [employees.id==positions.employee_id]
```

```prql
from employees
join side:left p=positions [employees.id==p.employee_id]
```

## Self equality operator

If the join conditions are of form `left.x == right.x`, we can use "self
equality operator":

```prql
from employees
join positions [==emp_no]
```
