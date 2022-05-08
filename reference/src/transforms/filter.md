# Filter

`filter` picks rows based on their values

```prql_no_test
filter {boolean_expression}
```

## Examples

```prql
from employees
filter age > 25
```

```prql
from employees
filter (age | in 25..40)
```
