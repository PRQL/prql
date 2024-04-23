# Join

Adds columns from another relation, matching rows based on a condition.

```prql no-eval
join side:{inner|left|right|full} rel (condition)
```

## Parameters

- `side` specifies which rows to include, defaulting to `inner`.
- `rel` - the relation to join with, possibly including an alias, e.g.
  `a=artists`.
- `condition` - the criteria on which to match the rows from the two relations.
  Theoretically, `join` will produce a cartesian product of the two input
  relations and then filter the result by the condition. It supports two
  additional features:
  - _Names [`this` & `that`](../../syntax/keywords.md#this--that)_: Along name
    `this`, which refers to the first input relation, `condition` can use name
    `that`, which refers to the second input relation.
  - _Self equality operator_: If the condition is an equality comparison between
    two columns with the same name (i.e. `(this.col == that.col)`), it can be
    expressed with only `(==col)`.

## Examples

```prql
from employees
join side:left positions (employees.id==positions.employee_id)
```

---

```prql
from employees
join side:left p=positions (employees.id==p.employee_id)
```

---

```prql
from tracks
join side:left artists (
  # This adds a `country` condition, as an alternative to filtering
  artists.id==tracks.artist_id && artists.country=='UK'
)
```

---

In SQL, CROSS JOIN is a join that returns each row from first relation matched
with all rows from the second relation. To accomplish this, we can use condition
`true`, which will return all rows of the cartesian product of the input
relations:

```
from shirts
join hats true
```

---

[`this` & `that`](../../syntax/keywords.md#this--that) can be used to refer to
the current & other table respectively:

```prql
from tracks
join side:inner artists (
  this.id==that.artist_id
)
```

---

If the join conditions are of form `left.x == right.x`, we can use "self
equality operator":

```prql
from employees
join positions (==emp_no)
```
