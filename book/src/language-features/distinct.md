# Distinct

PRQL doesn't have a specific `distinct` keyword. Instead, use `group` and `take 1`:

```prql
from employees
select department
group department (
  take 1
)
```

This also works without a linebreak:

```prql
from employees
select department
group department (take 1)
```

## Selecting from each group

We are be able to [select a rows from each
group](https://stackoverflow.com/questions/3800551/select-first-row-in-each-group-by-group)
by combining `group` and `sort`:

```prql
# youngest employee from each department
from employees
group department (
  sort age
  take 1
)
```

## Roadmap

When using Postgres dialect, we are planning to compile:

```prql_no_test
# youngest employee from each department
from employees
group department (
  sort age
  take 1
)
```

... to ...

```sql
SELECT DISTINCT ON (department) *
FROM employees
ORDER BY department, age
```
