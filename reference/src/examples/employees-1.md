Task:

> The company active pursues gender equality.
> 
> Prepare an analysis based on salaries and gender distribution by departments.

```prql
from employees
join salaries [emp_no]
group [emp_no, gender] (
  aggregate [
    emp_salary: average salary
  ]
)
join de:dept_emp [emp_no] side:left
group [de.dept_no, gender] (
  aggregate [
    salary_avg: average emp_salary,
    salary_sd: stddev emp_salary,
  ]
)
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
    de.dept_no,
    gender,
    AVG(emp_salary) AS salary_avg,
    STDDEV(emp_salary) AS salary_sd
  FROM
    table_0
    LEFT JOIN dept_emp AS de USING(emp_no)
  GROUP BY
    de.dept_no,
    gender
)
SELECT
  dept_name,
  gender,
  salary_avg,
  salary_sd
FROM
  table_1
  JOIN departments USING(dept_no)
```
