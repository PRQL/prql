# From

Specifies a data source.

```prql
from db.artists
```

To introduce an alias, use an assign expression:

```prql
from e = db.employees
select e.first_name
```

Table names containing spaces or special characters
[need to be contained within backticks](../../syntax/keywords.md#quoting):

```prql
from db.`artist tracks`
```
