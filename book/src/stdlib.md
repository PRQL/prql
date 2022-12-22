# Stdlib

The standard library is currently fairly limited, and we're very to expanding
it. If we find ourselves using [s-strings](./language-features/s-strings.md) for
something frequently, raise an issue and we'll add it to the stdlib.

```admonish note
Currently the stdlib implementation doesn't support different DB implementations itself;
those need to be built deeper into the compiler. We'll resolve this at some point. Until
then, we'll only add functions here that are broadly supported by most DBs.
```

Here's the source of the current
[PRQL stdlib](https://github.com/prql/prql/blob/main/prql-compiler/src/semantic/stdlib.prql):

```prql_no_test
{{#include ../../prql-compiler/src/semantic/stdlib.prql}}
```

And a couple of examples:

```prql
from employees
derive [
  gross_salary = (salary + payroll_tax | as int),
  gross_salary_rounded = (gross_salary | round 0),
]
```
