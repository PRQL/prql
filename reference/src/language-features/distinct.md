# Distinct

PRQL doesn't have a specific `distinct` keyword. Instead, use `group` and `take 1`:

```prql
from employees
select department
group department (
  take 1
)
```

or without a linebreak:
```prql
from employees
select department
group department ( | take 1)
```

> The `|` is here temporarily, until we finish work on function currying and how pipelines are treated.

Note that `group` can contain `sort`:

```prql
# youngest employees from each department
from employees
group department (
  sort age
  take 1
)
```
