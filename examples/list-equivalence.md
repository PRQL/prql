```elm
from employees
select salary
```

```sql
SELECT salary FROM employees
```

---

```elm
# Same as above but with `salary` in a list
from employees
select [salary]
```

```sql
SELECT salary FROM employees
```

---

```elm
from employees
derive [
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost
]
```

```sql
SELECT TOP 20
    *,
    salary + payroll_tax AS gross_salary,
    salary + payroll_tax + benefits_cost AS gross_cost
FROM employees
```

---

(not yet working)

```elm
# Same as above but split into two lines
from employees
derive gross_salary salary + payroll_tax
derive gross_cost gross_salary + benefits_cost
```

```sql
SELECT TOP 20
    title,
    country,
    salary + payroll_tax AS gross_salary,
    salary + payroll_tax + benefits_cost AS gross_cost
FROM employees
```
