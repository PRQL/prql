# Ranges

PRQL has a concise range syntax `start..end`. If only one of `start` & `end` are
supplied, the range is open on the empty side.

Ranges can be used in filters with the `in` function, with any type of literal,
including dates:

```prql
from events
filter (date | in @1776-07-04..@1787-09-17)
filter (magnitude | in 50..100)
derive is_northern = (latitude | in 0..)
```

Like in SQL, ranges are inclusive.

As discussed in the [take](../transforms/take.md) docs, ranges can also be used
in `take`:

```prql
from orders
sort [-value, date]
take 101..110
```

```admonish note
Half-open ranges are generally less intuitive to read than a simple `>=` or `<=`
operator.
```

## Roadmap

We'd like to use ranges for other types, such as whether an object is in an
array or list literal.
