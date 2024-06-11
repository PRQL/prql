# PRQL Language Book

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simple, powerful, pipelined
SQL replacement. Like SQL, it's readable, explicit and declarative. Unlike SQL,
it forms a logical pipeline of transformations, and supports abstractions such
as variables and functions. It can be used with any database that uses SQL,
since it compiles to SQL.

This book serves as a tutorial and reference guide on the language and the
broader project. It currently has three sections, navigated by links on the
left:

- **Tutorial** — A friendly & accessible guide for learning PRQL. It has a
  gradual increase of difficulty and requires only basic understanding of
  programming languages. Knowledge of SQL is beneficial, because of many
  comparisons to SQL, but not required.
- **Reference** — In-depth information about the PRQL language. Includes
  justifications for language design decisions and formal specifications for
  parts of the language.
- **Project** — General information about the project, tooling and development.

---

**Examples of PRQL** with a comparison to the generated SQL. PRQL queries can be
as simple as:

```prql
from tracks
filter artist == "Bob Marley"  # Each line transforms the previous result
aggregate {                    # `aggregate` reduces each column to a value
  plays    = sum plays,
  longest  = max length,
  shortest = min length,       # Trailing commas are allowed
}
```

...and here's a larger example:

```prql
from employees
filter start_date > @2021-01-01            # Clear date syntax
derive {                                   # `derive` adds columns / variables
  gross_salary = salary + (tax ?? 0),      # Terse coalesce
  gross_cost = gross_salary + benefits,    # Variables can use other variables
}
filter gross_cost > 0
group {title, country} (                   # `group` runs a pipeline over each group
  aggregate {                              # `aggregate` reduces each group to a value
    average gross_salary,
    sum_gross_cost = sum gross_cost,       # `=` sets a column name
  }
)
filter sum_gross_cost > 100_000            # `filter` replaces both of SQL's `WHERE` & `HAVING`
derive id = f"{title}_{country}"           # F-strings like Python
derive country_code = s"LEFT(country, 2)"  # S-strings permit SQL as an escape hatch
sort {sum_gross_cost, -country}            # `-country` means descending order
take 1..20                                 # Range expressions (also valid as `take 20`)
```
