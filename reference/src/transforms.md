# Transforms

Core principle of the language is a pipeline, which is a series of sequential transformations of a table (or data frame). There is only a few different types of transformations:

## From

> specifies a data source

```prql_no_test
from {table reference}
```

#### Examples

```prql
from employees
```

## Select

> picks columns based on their names

```prql_no_test
select [{expression}]
```

#### Examples

```prql
from employees
select [first_name, last_name]
```

## Derive

> adds new columns that are computed from existing columns

```prql_no_test
derive [{new_name} = {expression}]
```

#### Examples

```prql
from employees
derive gross_salary = salary + payroll_tax
```

```prql
from employees
derive [
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost
]
```

## Filter

> picks rows based on their values

```prql_no_test
filter {boolean expression}
```

#### Examples

```prql_no_test
from employees
filter (length last_name < 3)
```

## Sort

`sort` orders rows based on the values of one or more columns.

```prql_no_test
sort [{direction}{column}]
```

#### Arguments

- One or multiple columns
- Each column can be prefixed with:
  - `+`, for ascending order, the default
  - `-`, for descending order

#### Examples

```prql
from employees
sort age
```

```prql
from employees
sort (-age)
```

> Note that `sort -age` is not valid; `-age` needs to be surrounded by parentheses.

```prql
from employees
sort [age, -tenure]
```

## Take

> picks first n rows

```prql_no_test
take {n}
```

#### Examples

```prql
from employees
take 10
```

## Join

> adds columns from another table, matching rows based on a condition

```prql_no_test
join side:{inner|left|right|full} {table} {[conditions]}
```

#### Arguments

- `side` decides which rows to include. Defaults to `inner`
- table reference
- list of conditions
  - If all terms are column identifiers, this will compile to `USING(...)`. In this case, both of the tables must have specified column. The result will only contain one column instead one for each table.

#### Examples

```prql
from employees
join side:left positions [id==employee_id]
```

#### Examples

```prql
from employees
join side:full positions [emp_no]
```

## Group

A `group` transform maps a pipeline over a number of groups. The groups are determined by the
columns passed to `group`'s first argument.

The most conventional use of `group` is with `aggregate`:

```prql
from employees
group [title, country] (
  aggregate [
    average salary,
    ct = count
  ]
)
```

In concept, a transform in context of a `group` does the same transformation to the group as
it would to the table — for example finding the employee who joined first:

```prql
from employees
sort join_date
take 1
```

To find the employee who joined first in each department, it's exactly the
same pipeline, but within a `group` expression:

> Not yet implemented, ref <https://github.com/prql/prql/issues/421>

```prql_no_test
from employees
group role (
  sort join_date  # taken from above
  take 1
)
```

## Aggregate

> group rows by one or more columns

```prql_no_test
aggregate [{expression or assign operations}]
```

#### Examples

```prql
from employees
aggregate [
  average salary,
  ct = count
]
```
