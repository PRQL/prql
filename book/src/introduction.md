# Introduction

PRQL is a modern language for transforming data — a simple, powerful, pipelined
SQL replacement. Like SQL, it's readable, explicit and declarative. Unlike SQL,
it forms a logical pipeline of transformations, and supports abstractions such
as variables and functions. It can be used with any database that uses SQL,
since it transpiles to SQL.

Let's get started with an example:

<!-- TODO: make this onramp friendlier: https://github.com/PRQL/prql/issues/522 -->

```prql
from employees
filter start_date > @2021-01-01               # Clear date syntax
derive [                                      # `derive` adds columns / variables
  gross_salary = salary + (tax ?? 0),         # Terse coalesce
  gross_cost = gross_salary + benefits_cost,  # Variables can use other variables
]
filter gross_cost > 0
group [title, country] (                      # `group` runs a pipeline over each group
  aggregate [                                 # `aggregate` reduces each group to a value
    average gross_salary,
    sum_gross_cost = sum gross_cost,          # `=` sets a column name
  ]
)
filter sum_gross_cost > 100_000                # `filter` replaces both of SQL's `WHERE` & `HAVING`
derive id = f"{title}_{country}"              # F-strings like python
derive country_code = s"LEFT(country, 2)"     # S-strings allow using SQL as an escape hatch
sort [sum_gross_cost, -country]               # `-country` means descending order
take 1..20                                    # Range expressions (also valid here as `take 20`)
```

As you can see, PRQL is a linear **pipeline of transformations** — each line of
the query is a transformation of the previous line's result.

You can see that in SQL, operations do not follow one another, which makes it
hard to compose larger queries.
