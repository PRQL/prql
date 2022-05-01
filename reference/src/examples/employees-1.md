Task:

> The company active pursues gender equality.
>
> Prepare an analysis based on salaries and gender distribution by departments.

```prql
from employees
join salaries [emp_no]
group [emp_no, gender] (
  aggregate [
    emp_salary = average salary
  ]
)
join de=dept_emp [emp_no] side=left
group [de.dept_no, gender] (
  aggregate [
    salary_avg = average emp_salary,
    salary_sd = stddev emp_salary,
  ]
)
join departments [dept_no]
select [dept_name, gender, salary_avg, salary_sd]
```
