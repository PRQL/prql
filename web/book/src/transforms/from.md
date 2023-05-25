# From

Specifies a data source.

```prql no-eval
from table_reference
```

Table names containing
[schemas](../syntax/quoted-identifiers.md#quoting-schemas) or needing to be
quoted for [other reasons](../syntax/quoted-identifiers.md) need to be contained
within backticks.

## Examples

```prql
from employees
```

To introduce an alias, use an assign expression:

```prql
from e = employees
select e.first_name
```
