# Group

Partitions the rows into groups and applies a pipeline to each of the groups.

```prql no-eval
group {key_columns} (pipeline)
```

The partitioning of groups are determined by the `key_column`s (first argument).

The most conventional use of `group` is with `aggregate`:

```prql
from employees
group {title, country} (
  aggregate {
    average salary,
    ct = count salary
  }
)
```

In concept, a transform in context of a `group` does the same transformation to
the group as it would to the table — for example finding the employee who joined
first across the whole table:

```prql
from employees
sort join_date
take 1
```

To find the employee who joined first in each department, it's exactly the same
pipeline, but within a `group` expression:

```prql
from employees
group role (
  sort join_date  # taken from above
  take 1
)
```
