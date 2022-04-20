# PRQL-python

> PRQL-python exposes the prql_compiler rust package via an exposed python method `to_sql(query:str)->str`


#### Hypothetical Installation 
`pip install prql-python`
#### Usage 

```elm
prql_query = '''
from employees
join salaries [emp_no]
aggregate by:[emp_no, gender] [
  emp_salary: average salary
]
join departments [dept_no]
'''
```
```python
import prql_python as prql
prql.to_sql(prql_query)
```

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.  

```rust
#[pyfunction]
pub fn to_sql(query: &str) -> PyResult<String> {
```
