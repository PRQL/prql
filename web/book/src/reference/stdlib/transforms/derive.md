# Derive

Computes one or more new columns.

```prql no-eval
derive {
  name = expression,
  # or
  column,
}
```

## Examples

```prql
from.employees
derive gross_salary = salary + payroll_tax
```

```prql
from.employees
derive {
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost
}
```
