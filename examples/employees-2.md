```elm
# Task: The company active pursues gender equality.
# Prepare an analysis based on salaries and gender distribution by managers.

from employees
join salaries [emp_no]
aggregate by:[emp_no, gender] [
  emp_salary: average salary
]
join dept_emp [emp_no]
join dept_manager [
  (dept_manager.dept_no = dept_emp.dept_no) AND s"(dept_emp.from_date, dept_emp.to_date) OVERLAPS (dept_manager.from_date, dept_manager.to_date)"
]
aggregate by:[dept_manager.emp_no, gender] [
  salary_avg: average emp_salary,
  salary_sd: stddev emp_salary
]
derive [
  mng_no: dept_manager.emp_no
]
join employees [emp_no]
derive [mng_name: s"employees.first_name || ' ' || employees.last_name"]
select [mng_name, table_1.gender, salary_avg, salary_sd]
```

```elm
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
    dept_manager.emp_no,
    gender,
    AVG(emp_salary) AS salary_avg,
    STDDEV(emp_salary) AS salary_sd,
    dept_manager.emp_no AS mng_no
  FROM
    table_0
    JOIN dept_emp USING(emp_no)
    JOIN dept_manager ON dept_manager.dept_no = dept_emp.dept_no
    AND (dept_emp.from_date, dept_emp.to_date) OVERLAPS (dept_manager.from_date, dept_manager.to_date)
  GROUP BY
    dept_manager.emp_no,
    gender
),
table_2 AS (
  SELECT
    employees.first_name || ' ' || employees.last_name,
    table_1.gender,
    salary_avg,
    salary_sd
  FROM
    table_1
    JOIN employees USING(emp_no)
)
SELECT
  *
FROM
  table_2```
