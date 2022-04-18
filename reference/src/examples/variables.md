```prql
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
  title,
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
LIMIT
  20
```

```prql
from employees
aggregate by:[emp_no] [
  emp_salary: average salary          # avg_salary should resolve to "AVG(salary)" (from stdlib)
]
join titles [emp_no]
aggregate by:[title] [
  avg_salary: average emp_salary
]
select [salary_k: avg_salary / 1000]  # avg_salary should resolve to "AVG(emp_salary)"
take 10                               # induces new SELECT
derive [salary: salary_k * 1000]      # salary_k should not resolve to "avg_salary / 1000"
```

```sql
WITH table_0 AS (
  SELECT
    emp_no,
    AVG(salary) AS emp_salary
  FROM
    employees
  GROUP BY
    emp_no
)
SELECT
  AVG(emp_salary) / 1000 AS salary_k,
  AVG(emp_salary) / 1000 * 1000 AS salary
FROM
  table_0
  JOIN titles USING(emp_no)
GROUP BY
  title
LIMIT
  10
```
