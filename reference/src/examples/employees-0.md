Task:

> rank the employee titles according to the average salary for each department.

My solution:
- for each employee, find their average salary,
- join employees with their departments and titles (duplicating employees for each of their titles and departments)
- group by department and title, aggregating average salary
- join with department to get department name

```prql
from salaries
group [emp_no] (
  aggregate [emp_salary: average salary]
)
join t:titles [emp_no]
join dept_emp side:left [emp_no]
group [dept_emp.dept_no, t.title] (
  aggregate [avg_salary: average emp_salary]
)
join departments [dept_no]
select [dept_name, title, avg_salary]
```
