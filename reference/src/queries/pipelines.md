# Pipelines

### The simplest pipeline

The simplest pipeline is just:

```prql
from employees
```

### Adding transformations

We can add additional lines, each one transforms the result:

```prql
from employees
derive [gross_salary: (salary + payroll_tax)]
```

...and so on:

```prql
from employees
derive [gross_salary: (salary + payroll_tax)]
sort gross_salary
```
