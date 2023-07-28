# Ranges

Range `start..end` represents as set of values between `start` and `end`,
inclusive (greater of equal to `start` and less than or equal to `end`).

To express a range that is open on one side, either `start` or `end` can be
omitted.

Ranges can be used in filters with the `in` function, with any type of literal,
including dates:

```prql
from events
filter (created_at | in @1776-07-04..@1787-09-17)
filter (magnitude | in 50..100)
derive is_northern = (latitude | in 0..)
```

Ranges can also be used in `take`:

```prql
from orders
sort {-value, created_at}
take 101..110
```

```admonish note
Half-open ranges are generally less intuitive to read than a simple `>=` or `<=`
operator.
```

## See also

- [take transform](../stdlib/transforms/take.md)

## Roadmap

We'd like to use ranges for other types, such as whether an object is in an
array or list literal.
