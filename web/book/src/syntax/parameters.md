# Parameters

PRQL will retain parameters like `$1` in SQL output, which can then be supplied
to the SQL query as a prepared query:

```prql
from employees
filter id == $1
```
