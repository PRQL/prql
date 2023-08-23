## Single item is coerced into a list

```prql
from employees
select salary
```

Same as above but with `salary` in a list:

```prql
from employees
select {salary}
```

## Multiple items

```prql
from employees
derive {
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost
}
```

Same as above but split into two lines:

```prql
from employees
derive gross_salary = salary + payroll_tax
derive gross_cost = gross_salary + benefits_cost
```
