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

## Forthcoming features

Soon we'll be able to [select range of rows in each
group](https://stackoverflow.com/questions/3800551/select-first-row-in-each-group-by-group)
by combining `group` and `sort`:

```prql_no_test
# youngest employee from each department
from employees
group department (
  sort age
  take 1
)
```

... which will produces ...

```sql
WITH table_0 = (
  SELECT ROW_NUMBER() OVER(PARTITION BY department ORDER_BY age) as _row_number
  FROM employees
)
SELECT *
FROM table_0
WHERE _row_number = 1
```

... or in Postgres dialect ...

```sql
SELECT DISTINCT ON (department) *
FROM employees
ORDER BY department, age
```

Until this is implemented, we you can use `row_number`:

```prql
from employees
group department (
  sort age
  derive rank = row_number
)
filter rank == 1
```
