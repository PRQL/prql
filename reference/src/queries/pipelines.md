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
derive gross_salary = (salary + payroll_tax)
```

...and so on:

```prql
from employees
derive gross_salary = (salary + payroll_tax)
sort gross_salary
```

PRQL compiler will try to represent as many transforms as possible with a single `SELECT` statement, and uses CTE (common table expression) for complex pipelines:

```prql
from e = employees
derive gross_salary = (salary + payroll_tax)
sort gross_salary
take 10
join d = department [dept_no]
select [e.name, gross_salary, d.name]
```
