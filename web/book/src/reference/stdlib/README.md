# Standard library

The standard library currently contains commonly used functions that are used in
SQL. It's not yet as broad as we'd like, and we're very open to expanding it.

Currently s-strings are an escape-hatch for any function that isn't in our
standard library. If we find ourselves using them for something frequently,
raise an issue and we'll add it to the stdlib.

Here's the source of the current
[PRQL `std`](https://github.com/PRQL/prql/blob/main/prqlc/prqlc/src/semantic/std.prql):

```admonish note
PRQL 0.9.0 has started supporting different DB implementations for standard library functions.
The source is the [`std.sql`](https://github.com/PRQL/prql/blob/main/prqlc/prqlc/src/sql/std.sql.prql).
```

```prql no-eval
{{#include ../../../../../prqlc/prqlc/src/semantic/std.prql}}
```

And a couple of examples:

```prql
from db.employees
derive {
  gross_salary = (salary + payroll_tax | as int),
  gross_salary_rounded = (gross_salary | math.round 0),
  time = s"NOW()",  # an s-string, given no `now` function exists in PRQL
}
```

Example of different implementations of division and integer division:

```prql
prql target:sql.sqlite

[{x = 13, y = 5}]
select {
  quotient = x / y,
  int_quotient = x // y,
}
```

```prql
prql target:sql.mysql

[{x = 13, y = 5}]
select {
  quotient = x / y,
  int_quotient = x // y,
}
```
