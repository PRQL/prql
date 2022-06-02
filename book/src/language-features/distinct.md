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
group department ( | take 1) # Note below
```

> Note: `|` is here temporarily, until we finish work on function & pipeline currying.


-----

We are be able to [select range of rows in each
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
