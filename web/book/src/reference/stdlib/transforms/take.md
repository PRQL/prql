# Take

Picks rows based on their position.

```prql no-eval
take (n|range)
```

See [Ranges](../../syntax/ranges.md) for more details on how ranges work.

## Examples

```prql
from.employees
take 10
```

```prql
from.orders
sort {-value, created_at}
take 101..110
```
