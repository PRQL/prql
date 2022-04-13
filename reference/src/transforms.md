# Transforms

Core principle of the language is a pipeline, which is a series of sequential transformations of a table (or data frame). There is only a few different types of transformations:

## From

> specifies a data source

```prql-no-test
from {table reference}
```

*Example:*

```prql
from employees
```

## Select

> picks columns based on their names

```prql-no-test
select [{expression}]
```

*Example:*

```prql
from employees
select [first_name, last_name]
```

## Derive

> adds new columns that are computed from existing columns

```prql-no-test
derive [{new_name}: {expression}]
```

*Example:*

```prql
from employees
derive gross_salary: salary + payroll_tax
```

```prql
from employees
derive [
  gross_salary: salary + payroll_tax,
  gross_cost: gross_salary + benefits_cost
]
```

## Filter

> picks rows based on their values

```prql-no-test
filter {boolean expression}
```

*Example:*

```prql-no-test
from employees
filter (length last_name < 3)
```

## Take

> picks first n rows

```prql-no-test
take {n}
```

*Example:*

```prql
from employees
take 10
```

## Sort

> orders the rows by the values of selected columns

```prql-no-test
sort {column}
```

*Arguments:*

- a column identifier of the key to sort by

*Example:*

```prql
from employees
sort age
```

## Join

> adds columns from another table, matching rows based on a condition

```prql-no-test
join side:{inner|left|right|full} {table} {[conditions]}
```

*Arguments:*

- `side` decides which rows to include. Defaults to `inner`
- table reference
- list of conditions
  - If all terms are column identifiers, this will compile to `USING(...)`. In this case, both of the tables must have specified column. The result will only contain one column instead one for each table.

*Example:*

```prql
from employees
join side:left positions [id=employee_id]
```

*Example:*

```prql
from employees
join side:full positions [emp_no]
```

## Aggregate

> group rows by one or more columns

```prql-no-test
aggregate by:[{column identifier}] [{expression or assign operation}]
```

*Example:*

```prql
from employees
aggregate by:[title, country] [
  average salary,
  ct: count
]
```
