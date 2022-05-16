# Distinct

PRQL doesn't have a specific `distinct` keyword. Instead, use `aggregate`:

```prql
from employees
aggregate first_name by:first_name
```

...produces...
