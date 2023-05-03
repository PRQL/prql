# Select

Picks and computes columns.

```prql no-eval
select [
  {name} = {expression},
  # or
  {column},
]
# or
select ![{column}]
```

## Examples

```prql
from employees
select name = f"{first_name} {last_name}"
```

```prql
from employees
select [
  name = f"{first_name} {last_name}",
  age_eoy = dob - @2022-12-31,
]
```

```prql
from employees
select first_name
```

```prql
from e=employees
select [e.first_name, e.last_name]
```

### Excluding columns

We can use `!` to exclude a list of columns. This can operate in two ways:

- We use `SELECT * EXCLUDE` / `SELECT * EXCEPT` for the columns supplied to
  `select ![]` in dialects which support it.
- Otherwise, the columns must have been defined prior in the query (unless all
  of a table's columns are excluded); for example in another `select` or a
  `group` transform. In this case, we evaluate and specify the columns that
  should be included in the output SQL.

Some examples:

```prql no-fmt
prql target:sql.bigquery
from tracks
select ![milliseconds,bytes]
```

```prql no-fmt
from tracks
select [track_id, title, composer, bytes]
select ![title, composer]
```

```prql no-fmt
from artists
derive nick = name
select ![artists.*]
```

<!-- TODO: I think this should move to a separate "Aliases" page -->

````admonish note
In the final example above, the `e` representing the table / namespace is no
longer available after the `select` statement. For example, this would raise an error:

```prql no-eval
from e=employees
select e.first_name
filter e.first_name == "Fred" # Can't find `e.first_name`
```

To refer to the `e.first_name` column in subsequent transforms,
either refer to it using `first_name`, or if it requires a different name,
assign one in the `select` statement:

```prql
from e=employees
select fname = e.first_name
filter fname == "Fred"
```
````
