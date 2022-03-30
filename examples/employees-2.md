```elm
# Task: The company active pursues gender equality.
# Prepare an analysis based on salaries and gender distribution by managers.

from employees
join salaries [emp_no]
aggregate by:[emp_no, gender] [
  emp_salary: average salary
]
join de:dept_emp [emp_no]
join dm:dept_manager [
  (dm.dept_no = de.dept_no) and s"(de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)"
]
aggregate by:[dm.emp_no, gender] [
  salary_avg: average emp_salary,
  salary_sd: stddev emp_salary
]
derive [
  mng_no: dm.emp_no
]
join managers:employees [emp_no]
derive [mng_name: s"managers.first_name || ' ' || managers.last_name"]
select [mng_name, managers.gender, salary_avg, salary_sd]
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
    dm.emp_no,
    gender,
    AVG(emp_salary) AS salary_avg,
    STDDEV(emp_salary) AS salary_sd,
    dm.emp_no AS mng_no
  FROM
    table_0
    JOIN dept_emp AS de USING(emp_no)
    JOIN dept_manager AS dm ON dm.dept_no = de.dept_no
    AND (de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)
  GROUP BY
    dm.emp_no,
    gender
)
SELECT
  managers.first_name || ' ' || managers.last_name,
  managers.gender,
  salary_avg,
  salary_sd
FROM
  table_1
  JOIN employees AS managers USING(emp_no)
```
