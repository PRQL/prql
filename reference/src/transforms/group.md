## Group

A `group` transform maps a pipeline over a number of groups. The groups are determined by the
columns passed to `group`'s first argument.

The most conventional use of `group` is with `aggregate`:

```prql
from employees
group [title, country] (
  aggregate [
    average salary,
    ct = count
  ]
)
```

In concept, a transform in context of a `group` does the same transformation to the group as
it would to the table — for example finding the employee who joined first:

```prql
from employees
sort join_date
take 1
```

To find the employee who joined first in each department, it's exactly the
same pipeline, but within a `group` expression:

> Not yet implemented, ref <https://github.com/prql/prql/issues/421>

```prql_no_test
from employees
group role (
  sort join_date  # taken from above
  take 1
)
```
