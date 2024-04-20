# Null handling

SQL has an unconventional way of handling `NULL` values, since it treats them as
unknown values. As a result, in SQL:

- `NULL` is not a value indicating a missing entry, but a placeholder for
  anything possible,
- `NULL = NULL` evaluates to `NULL`, since one cannot know if one unknown is
  equal to another unknown,
- `NULL <> NULL` evaluates to `NULL`, using same logic,
- to check if a value is `NULL`, SQL introduces `IS NULL` and `IS NOT NULL`
  operators,
- `DISTINCT column` may return multiple `NULL` values.

For more information, check out the
[Postgres documentation](https://www.postgresql.org/docs/current/functions-comparison.html).

PRQL, on the other hand, treats `null` as a value, which means that:

- `null == null` evaluates to `true`,
- `null != null` evaluates to `false`,
- distinct column cannot contain multiple `null` values.

```prql
from employees
filter first_name == null
filter null != last_name
```

Note that PRQL doesn't change how `NULL` is compared between columns, for
example in joins. (PRQL compiles to SQL and so can't change the behavior of the
database).

For more context or to provide feedback check out the discussion on
[issue #99](https://github.com/PRQL/prql/issues/99).
