# Select

Picks and computes columns.

```prql no-eval
select {
  name = expression,
  # or
  column,
}
# or
select !{column}
```

## Examples

```prql
from.employees
select name = f"{first_name} {last_name}"
```

```prql
from.employees
select {
  name = f"{first_name} {last_name}",
  age_eoy = dob - @2022-12-31,
}
```

```prql
from.employees
select first_name
```

```prql
from.employees
select {e = this}
select {e.first_name, e.last_name}
```

### Excluding columns

We can use `!` to exclude a list of columns. This can operate in two ways:

- We use `SELECT * EXCLUDE` / `SELECT * EXCEPT` for the columns supplied to
  `select !{}` in dialects which support it.
- Otherwise, the columns must have been defined prior in the query (unless all
  of a table's columns are excluded); for example in another `select` or a
  `group` transform. In this case, we evaluate and specify the columns that
  should be included in the output SQL.

Some examples:

```prql
prql target:sql.bigquery
from.tracks
select !{milliseconds, bytes}
```

```prql
from.tracks
select {track_id, title, composer, bytes}
select !{title, composer}
```

```prql
from.artists
derive nick = name
select !{artists.*}
```

Note that `!` is also the `NOT` operator, so without the tuple it has a
different meaning:

```prql
prql target:sql.bigquery
from.tracks
select !is_compilation
```
