```elm
# Task: rank the employee titles according to the average salary for each department.

# My solution:
# - for each employee, find their average salary,
# - join employees with their departments and titles (duplicating employees for each of their titles and departments)
# - group by department and title, aggregating average salary
# - join with department to get department name

from salaries
aggregate by:[emp_no] [
  emp_salary: average salary
]
join titles [emp_no]
# TODO: add a `left` join
join dept_emp [emp_no]
aggregate by:[dept_emp.dept_no, titles.title] [
  avg_salary: average emp_salary
]
join departments [dept_no]
select [dept_name, title, avg_salary]

# Note that this solution contains a common pitfall in the first few lines:
# We don't need to use `employees` table at all, because we could only aggregate `salaries` table by `emp_no`.
```

```sql
WITH table_0 AS (
  SELECT
    emp_no,
    AVG(salary) AS emp_salary
  FROM
    salaries
  GROUP BY
    emp_no
),
table_1 AS (
  SELECT
    dept_emp.dept_no,
    titles.title,
    AVG(emp_salary) AS avg_salary
  FROM
    table_0
    JOIN titles USING(emp_no)
    JOIN dept_emp USING(emp_no)
  GROUP BY
    dept_emp.dept_no,
    titles.title
),
table_2 AS (
  SELECT
    dept_name,
    title,
    avg_salary
  FROM
    table_1
    JOIN departments USING(dept_no)
)
SELECT
  *
FROM
  table_2
```
