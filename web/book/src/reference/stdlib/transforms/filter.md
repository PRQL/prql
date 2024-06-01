# Filter

Picks rows based on their values.

```prql no-eval
filter boolean_expression
```

## Examples

```prql
from db.employees
filter age > 25
```

```prql
from db.employees
filter (age > 25 || department != "IT")
```

```prql
from db.employees
filter (department | in ["IT", "HR"])
```

```prql
from db.employees
filter (age | in 25..40)
```
