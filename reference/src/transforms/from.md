# From

Specifies a data source.

```prql_no_test
from {table_reference}
```

## Examples

```prql
from employees
```


To introduce an alias, use an assign expression:

```prql
from e = employees
select e.first_name
```
