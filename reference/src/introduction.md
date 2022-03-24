# Introduction

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

Let's get started with an example:

```prql
from employees
filter country_code = "USA"                # Each line transforms the previous result.
derive [                                   # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost: gross_salary + benefits_cost # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country_code] [       # `by` are the columns to group by.
  average salary,                          # These are aggregation calcs run on each group.
  sum     salary,
  average gross_salary,
  sum     gross_salary,
  average gross_cost,
  sum_gross_cost: sum gross_cost,
  ct: count *
]
sort sum_gross_cost
filter ct > 200
take 20
join countries side:left [country_code]
derive [
  always_true: true,
  s_string: s"version()",                  # An S-string, which transpiles directly into SQL
]
```

As you can see, PRQL is a linear **pipeline of transformations** — each line of the
query is a transformation of the previous line's result.

This query can be compiled into SQL:

```sql
WITH table_0 AS (
  SELECT
    TOP (20) title,
    country_code,
    SUM(salary + payroll_tax + benefits_cost) AS sum_gross_cost,
    COUNT(*) AS ct,
    AVG(salary),
    SUM(salary),
    AVG(salary + payroll_tax),
    SUM(salary + payroll_tax),
    AVG(salary + payroll_tax + benefits_cost)
  FROM
    employees
  WHERE
    country_code = 'USA'
    and salary + payroll_tax + benefits_cost > 0
  GROUP BY
    title,
    country_code
  HAVING
    ct > 200
  ORDER BY
    sum_gross_cost
),
table_1 AS (
  SELECT
    *,
    true AS always_true,
    version() AS s_string
  FROM
    table_0
    LEFT JOIN countries USING(country_code)
)
SELECT
  *
FROM
  table_1
```

You can see that in SQL, operations do not follow one another, which makes it hard to compose larger queries.
