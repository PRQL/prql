```elm
# Task: The company active pursues gender equality.
# Prepare an analysis based on salaries and gender distribution by departments.

from employees
join salaries [emp_no]
aggregate by:[emp_no, gender] [
  emp_salary: average salary
]
join dept_emp [emp_no]
aggregate by:[dept_no, gender] [
  salary_avg: average emp_salary,
  salary_sd: stddev emp_salary,
]
join departments [dept_no]
select [dept_name, gender, salary_avg, salary_sd]
```

```sql
WITH table_0 AS (
  SELECT
    emp_no,
    gender,
    AVG(salary) AS emp_salary
  FROM
    employees
    JOIN salaries USING(emp_no)
  GROUP BY
    emp_no,
    gender
),
table_1 AS (
  SELECT
    dept_no,
    gender,
    AVG(emp_salary) AS salary_avg,
    STDDEV(emp_salary) AS salary_sd
  FROM
    table_0
    JOIN dept_emp USING(emp_no)
  GROUP BY
    dept_no,
    gender
),
table_2 AS (
  SELECT
    dept_name,
    gender,
    salary_avg,
    salary_sd
  FROM
    table_1
    JOIN departments USING(dept_no)
)
SELECT
  *
FROM
  table_2
```
