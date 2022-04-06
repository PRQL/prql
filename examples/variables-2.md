```elm
from employees
aggregate by:[emp_no] [
  emp_salary: average salary          # avg_salary should resolve to "AVG(salary)" (from stdlib)
]
join titles [emp_no]
aggregate by:[title] [
  avg_salary: average emp_salary
]
select salary_k: avg_salary / 1000    # avg_salary should resolve to "AVG(emp_salary)"
take 10                               # induces new SELECT
derive salary: salary_k * 1000        # salary_k should not resolve to "avg_salary / 1000"
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
