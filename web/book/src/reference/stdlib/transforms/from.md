# From

Specifies a data source.

```prql no-eval
from table_reference
```

Table names containing schemas, keywords, or special characters
[need to be contained within backticks](../../syntax/keywords.md#quoting).

## Examples

```prql
from employees
```

To introduce an alias, use an assign expression:

```prql
from e = employees
select e.first_name
```
