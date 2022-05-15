# Coalesce

We can coalesce values with an `??` operator. Coalescing takes either the first
value or, if that value is null, the second value.

```prql
from orders
derive amount ?? 0
```
