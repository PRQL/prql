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
filter col1 == col2
```

  ...is equivalent to:

```prql
from tbl | select [col1, col2] | filter col1 == col2
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

### Punctuation summary

A summary of how PRQL uses punctuation

| Syntax   | Usage                   | Example                                                                      |
| -------- | ----------------------- | ---------------------------------------------------------------------------- |
| `:`      | Named args & Parameters | `interp lower:0 1600 sat_score`                                              |
| `=`      | Assigns & Aliases       | `derive temp_c = (temp_f | celsius_of_fahrenheit)` <br> `from e = employees` |
| `==`     | Equality comparison     | `join s=salaries [s.employee_id == employees.id]`                            |
| `->`     | Function definitions    | `func add a b -> a + b`                                                      |
| `<type>` | Annotations             | `@2021-01-01<datetime>`                                                      |
| `+`/`-`  | Sort order              | `sort [-amount, +date]`                                                      |
