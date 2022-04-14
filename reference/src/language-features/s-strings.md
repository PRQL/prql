# S-Strings

An s-string inserts SQL directly, as an escape hatch when there's something that PRQL
doesn't yet implement. For example, there's no `version()` function in SQL that
returns the Postgres version, so if we want to use that, we use an s-string:

```prql
derive db_version: s"version()"
```

We can embed columns in an s-string using braces. For example, PRQL's standard
library defines the `average` function as:

```prql_no_test
func average column = s"AVG({column})"
```

So this compiles using the function:

```prql
from employees
aggregate [average salary]
```

For those who have used python, it's similar in to a python f-string, but the
result is SQL, rather than a string literal — a python f-string
would produce `"average(salary)"`, with the quotes.
