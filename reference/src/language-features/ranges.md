# Ranges

PRQL has a concise range syntax `start..end`, which can be accessed with the `in` function:

```prql
from employees
filter (age | in 18..40)
```

Like in SQL, ranges are inclusive.

As discussed in the [take](../transforms/take.md) docs, ranges can also be used
in `take`:

```prql
from orders
sort [-value, date]
take 101..110
```
