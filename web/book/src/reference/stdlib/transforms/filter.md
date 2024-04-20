# Filter

Picks rows based on their values.

```prql no-eval
filter boolean_expression
```

## Examples

```prql
from employees
filter age > 25
```

```prql
from employees
filter (age > 25 || department != "IT")
```

```prql
from employees
filter (department | in ["IT", "HR"])
```

```prql
from employees
filter (age | in 25..40)
```
