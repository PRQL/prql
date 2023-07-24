```prql
from employees
filter country == "USA"                      # Each line transforms the previous result.
derive {                                     # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost  # Variables can use other variables.
}
filter gross_cost > 0
group {title, country} (                     # For each group use a nested pipeline
  aggregate {                                # Aggregate each group to a single row
    average salary,
    average gross_salary,
    sum salary,
    sum gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,
    ct = count this,
  }
)
sort sum_gross_cost
filter ct > 200
take 20
```

```prql
from employees
group {emp_no} (
  aggregate {
    emp_salary = average salary     # average salary resolves to "AVG(salary)" (from stdlib)
  }
)
join titles (==emp_no)
group {title} (
  aggregate {
    avg_salary = average emp_salary
  }
)
select salary_k = avg_salary / 1000 # avg_salary should resolve to "AVG(emp_salary)"
take 10                             # induces new SELECT
derive salary = salary_k * 1000     # salary_k should not resolve to "avg_salary / 1000"
```
