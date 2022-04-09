# Pipelines

### The simplest pipeline

The simplest pipeline is just:

```prql
from employees
```

...which is equivalent to:

```sql
SELECT * FROM employees
```

### Adding transformations

We can add additional lines, each one transforms the result:

```prql
from employees
derive gross_salary: (salary + payroll_tax)
```

...which is equivalent to:

```sql
SELECT
  *,
  salary + payroll_tax AS gross_salary
FROM employees
```

...and so on:

```prql
from employees
derive gross_salary: (salary + payroll_tax)
aggregate
sort gross_salary
```

...which is equivalent to:

```sql
SELECT
  *,
  salary + payroll_tax AS gross_salary
FROM employees
ORDER BY gross_salary
```

{{#include ../../../examples/variables-1.md}}
