# Take

Picks rows based on their position.

```prql_no_test
take {n|range}
```

See [Ranges](../language-features/ranges.md) for more details on how ranges
work.

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
