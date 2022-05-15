# Take

Picks rows based on their position.

```prql_no_test
take {n|range}
```

## Examples

```prql
from employees
take 10
```

```prql
from orders
sort [-value, date]
take 101..110
```
