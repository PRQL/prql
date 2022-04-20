# PRQL-python

> PRQL-python exposes the prql_compiler rust package via an exposed method `to_sql(query)`


```python
import prql_python as prql

prql.to_sql('''
```
```elm
    from employees
    join salaries [emp_no]
    aggregate by:[emp_no, gender] [
      emp_salary: average salary
    ]
    join de:dept_emp [emp_no] side:left
    aggregate by:[de.dept_no, gender] [
      salary_avg: average emp_salary,
      salary_sd: stddev emp_salary,
    ]
    join departments [dept_no]
    select [dept_''')
```
```python
''')
```
Uses the wonderful Rust crate [pyo3](https://github.com/PyO3/pyo3) .
