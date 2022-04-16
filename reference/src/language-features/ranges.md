# Ranges

PRQL has a concise range syntax, which can be accessed with the `in` function:

```prql
from employees
filter (age | in 18..40)
```
