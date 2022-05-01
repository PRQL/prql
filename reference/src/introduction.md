# Introduction

> [Note that this is a very early version and actively being worked on; thanks for
your patience]

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

Let's get started with an example:

<!-- TODO: resolve formatting — way too wide for the current preprocessor -->

```prql
from employees
filter country_code == "USA"   # Each line transforms the previous result.
derive [                       # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost  # Variables can use other variables.
]
filter gross_cost > 0
group [title, country_code] (  # For each group use a nested pipeline
  aggregate [                  # Aggregate each group to a single row
    average salary,
    average gross_salary,
    sum salary,
    sum gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,
    ct = count,
  ]
)
sort sum_gross_cost
filter ct > 200
take 20
join countries side=left [country_code]
derive [
  always_true = true,
  db_version = s"version()",    # An S-string, which transpiles directly into SQL
]
```

As you can see, PRQL is a linear **pipeline of transformations** — each line of the
query is a transformation of the previous line's result.

You can see that in SQL, operations do not follow one another, which makes it hard to compose larger queries.
