# Syntax

<!-- Here we could explain how function parameters work, what is a list, S-strings, how to do aliases and so on. -->

### Lists

- Most keywords that take a single argument can also take a list, so these are equivalent:

  ```diff
   from employees
  -select salary
  +select [salary]
  ```

- More examples in [**list-equivalence.md**](examples/list-equivalence.md).

### Pipelines

- A line-break generally creates a pipelined transformation. For example:

```prql
from tbl
select [
  col1,
  col2,
]
filter col1 = col2
```

  ...is equivalent to:

```prql
from tbl | select [col1, col2] | filter col1 = col2
```

- A line-break doesn't created a pipeline in a few cases:
  - Within a list (e.g. the `select` example above).
  - When the following line is a new statement, by starting with a keyword such
    as `func`.

### CTEs

- See [CTE Example](examples/cte.md).
- This is no longer point-free, but that's a feature rather than a requirement.
  The alternative is subqueries, which are fine at small scale, but become
  difficult to digest as complexity increases.
