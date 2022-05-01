Task:

> Find distributions of titles, salaries and genders over different departments.

```prql
from de=dept_emp
join s=salaries side=left [
  (s.emp_no == de.emp_no),
  s"({s.from_date}, {s.to_date}) OVERLAPS ({de.from_date}, {de.to_date})"
]
group [de.emp_no, de.dept_no] (
  aggregate salary = (average s.salary)
)
join employees [emp_no]
join titles [emp_no]
select [dept_no, salary, employees.gender, titles.title]
```
