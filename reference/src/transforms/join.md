# Join

Adds columns from another table, matching rows based on a condition.

```prql_no_test
join side:{inner|left|right|full} {table} {[conditions]}
```

## Arguments

- `side` decides which rows to include. Defaults to `inner`
- Table reference
- List of conditions
  - If all terms are column identifiers, this will compile to `USING(...)`. In
    this case, both of the tables must have specified column. The result will
    only contain one column for both tables.

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
