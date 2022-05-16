# Null handling

SQL has an unconventional way of handling `NULL` values, since it treats them as unknown values. In consequence:

- `NULL` is not a value indicating a missing entry, but a placeholder for anything possible,
- `NULL = NULL` evaluates to `NULL`, since one cannot know if one unknown is equal to another unknown,
- `NULL <> NULL` evaluates to `NULL`, using same logic,
- to check if a value is `NULL`, SQL introduces `IS NULL` and `IS NOT NULL` operators,
- `DISTINCT column` may return multiple `NULL` values.

For more information, read [Postgres documentation](https://www.postgresql.org/docs/current/functions-comparison.html).

PRQL, on the other hand, treats `null` as a value, which means that:

- `null == null` evaluates to `true`,
- `null != null` evaluates to `false`,
- distinct column cannot contain multiple `null` values.

This approach was discussed in [issue #99](https://github.com/prql/prql/issues/99).

> Note, currently `DISTINCT` is not yet implemented, see [#292](https://github.com/prql/prql/issues/292)

```prql
from employees
filter first_name == null
filter null != last_name
```