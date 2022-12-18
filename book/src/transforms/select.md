# Select

Picks and computes columns.

```prql_no_test
select [
  {new_name} = {expression},
  # or
  {expression}
]
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

_Note:_ In the example above, the `e` representing the table / namespace
is no longer available after the `select` statement.
So the `filter` statement after it displays a "Can't find" error:

```prql_no_test
from e=employees
select e.first_name
filter e.first_name == "Fred" # Can't find `e.first_name`
```

To refer to the `e.first_name` column in subsequent processing,
assign it a name in the `select` statement:

```prql
from e=employees
select fname = e.first_name
filter fname == "Fred" 
```
