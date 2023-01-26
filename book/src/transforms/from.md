# From

Specifies a data source.

```prql_no_test
from {table_reference}
```

Table names containing [schemas](../syntax.md#quoting-schemas) or needing to be
quoted for [other reasons](../syntax.md#quoted-identifiers) need to be contained
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
