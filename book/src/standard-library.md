# Standard Library

The standard library currently contains commonly used functions that are used in
SQL. It's not yet as broad as we'd like, and and we're very open to expanding
it.

Currently s-strings are an escape-hatch for any function that isn't in our
standard library. If we find ourselves using them for something frequently,
raise an issue and we'll add it to the stdlib.

```admonish note
Currently the stdlib implementation doesn't support different DB implementations itself;
those need to be built deeper into the compiler. We'll resolve this at some point. Until
then, we'll only add functions here that are broadly supported by most DBs.
```

Here's the source of the current
[PRQL `std`](https://github.com/PRQL/prql/blob/main/prql-compiler/src/semantic/std.prql):

```prql_no_test
{{#include ../../prql-compiler/src/semantic/std.prql}}
```

And a couple of examples:

```prql
from employees
derive [
  gross_salary = (salary + payroll_tax | as int),
  gross_salary_rounded = (gross_salary | round 0),
  time = s"NOW()",  # an s-string, given no `now` function exists in PRQL
]
```
