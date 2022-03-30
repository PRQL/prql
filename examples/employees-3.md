```elm
# Task: Find distributions of titles, salaries and genders over different departments.

from dept_emp
join salaries side:left [
  (salaries.emp_no = dept_emp.emp_no) and s"(salaries.from_date, salaries.to_date) OVERLAPS (dept_emp.from_date, dept_emp.to_date)"]
aggregate by:[dept_emp.emp_no, dept_emp.dept_no] [
  salary: average salaries.salary
]
join employees [emp_no]
join titles [emp_no]
select [dept_no, salary, employees.gender, titles.title]
```

```sql
WITH table_0 AS (
  SELECT
    dept_emp.emp_no,
    dept_emp.dept_no,
    AVG(salaries.salary) AS salary
  FROM
    dept_emp
    LEFT JOIN salaries ON salaries.emp_no = dept_emp.emp_no
    AND (salaries.from_date, salaries.to_date) OVERLAPS (dept_emp.from_date, dept_emp.to_date)
  GROUP BY
    dept_emp.emp_no,
    dept_emp.dept_no
)
SELECT
  dept_no,
  salary,
  employees.gender,
  titles.title
FROM
  table_0
  JOIN employees USING(emp_no)
  JOIN titles USING(emp_no)
```
