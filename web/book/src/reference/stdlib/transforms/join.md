# Join

Adds columns from another table, matching rows based on a condition.

```prql no-eval
join side:{inner|left|right|full} table (condition)
```

## Parameters

- `side` decides which rows to include, defaulting to `inner`.
- `table` - a reference to a relation, possibly including an assignment, e.g.
  `var= ...`
- `condition` - a boolean condition
  - If the condition evaluates to True, the rows will be joined
  - If name is the same from both tables, it can be expressed with only `==col`.

## Examples

```prql
from employees
join side:left positions (employees.id==positions.employee_id)
```

```prql
from employees
join side:left p=positions (employees.id==p.employee_id)
```

```prql
from tracks
join side:left artists (
  artists.id==tracks.artist_id && artists.country=='UK'
  # As an alternative to filtering
)
```

## Self equality operator

If the join conditions are of form `left.x == right.x`, we can use "self
equality operator":

```prql
from employees
join positions (==emp_no)
```
