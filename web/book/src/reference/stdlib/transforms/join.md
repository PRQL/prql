# Join

Adds columns from another table, matching rows based on a condition.

```prql no-eval
join side:{inner|left|right|full} table (condition)
```

## Parameters

- `side` specifies which rows to include, defaulting to `inner`.
- _table_ - a reference to a relation,
- _condition_ - a boolean condition
  - If the condition evaluates to true for a given row, the row will be joined
  - If name is the same from both tables, it can be expressed with only
    `(==col)`.

## Examples

```prql
from.employees
join side:left from.positions (employees.id==positions.employee_id)
```

```prql
from.tracks
join side:left from.artists (
  # This adds a `country` condition, as an alternative to filtering
  artists.id==tracks.artist_id && artists.country=='UK'
)
```

[`this` & `that`](../../syntax/keywords.md#this--that) can be used to refer to
the current & other table respectively:

```prql
from.tracks
join side:inner from.artists (
  this.id==that.artist_id
)
```

## Self equality operator

If the join conditions are of form `left.x == right.x`, we can use "self
equality operator":

```prql
from.employees
join from.positions (==emp_no)
```
