```prql
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
join (t:titles) [emp_no]
join dept_emp side:left [emp_no]
aggregate by:[dept_emp.dept_no, t.title] [
  avg_salary: average emp_salary
]
join departments [dept_no]
select [dept_name, title, avg_salary]
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
    t.title,
    AVG(emp_salary) AS avg_salary
  FROM
    table_0
    JOIN titles AS t USING(emp_no)
    LEFT JOIN dept_emp USING(emp_no)
  GROUP BY
    dept_emp.dept_no,
    t.title
)
SELECT
  dept_name,
  title,
  avg_salary
FROM
  table_1
  JOIN departments USING(dept_no)
```
