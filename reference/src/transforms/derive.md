# Derive

`derive` computes one or more new columns.

```prql_no_test
derive [{new_name} = {expression}]
```

## Examples

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

It generally computes a column from existing columns, but can also take a
literal, and doesn't require a `from`:

```prql
derive x = 5
```
