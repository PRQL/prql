# Numbers

Numbers can contain underscores between numbers; which can make reading large
numbers easier.

PRQL currently converts scientific notation into their full form.

```prql
from numbers
select {
    small = 1.000_000_1,
    big = 5_000_000,
    huge = 5e9,
}
```
