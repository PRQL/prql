```elm
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter ct > 200
take 20
```

```sql
SELECT
  TOP (20) title,
  country,
  AVG(salary),
  SUM(salary),
  AVG(salary + payroll_tax),
  SUM(salary + payroll_tax),
  AVG(salary + payroll_tax + benefits_cost),
  SUM(salary + payroll_tax + benefits_cost) AS sum_gross_cost,
  COUNT(*) AS ct
FROM
  employees
WHERE
  country = 'USA'
  AND salary + payroll_tax + benefits_cost > 0
GROUP BY
  title,
  country
HAVING
  COUNT(*) > 200
ORDER BY
  sum_gross_cost
```
