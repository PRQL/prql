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
[need to be contained within backticks](../syntax/keywords.md#quoting):

```prql
from db.`artist tracks`
```

## Table name conflicts

In the unlikely case that a table name matches a function from the standard
library, `default_db.tablename` can be used.

```admonish note
We realize this is an awkward workaround. Track & 👍 [#3271](https://github.com/PRQL/prql/issues/3271) for resolving this.
```

```prql
default_db.group  # in place of `from group`
take 1
```

## See also

- [Reading files](./read-files.md)
- [Relation literals](./relation-literals.md)
