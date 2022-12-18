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

Note that currently `select e.first_name` is an alias for `select first_name =
e.first_name`, such that the table / namespace is lost. So this would not work:

```prql_no_test
from e=employees
select e.first_name
select fn = e.first_name  # Error: cannot find `e.first_name`
```

...and would instead need to be:

```prql
from e=employees
select e.first_name
derive fn = first_name  # No `e.` here
```
