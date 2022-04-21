# prql-python

`prql-python` exposes the prql-compiler rust package via the python method `to_sql(query:str)->str`


#### Installation
`pip install prql-python`
#### Usage

```python
import prql_python as prql
prql_query = '''
    from employees
    join salaries [emp_no]
    aggregate by:[emp_no, gender] [
      emp_salary: average salary
    ]
    join departments [dept_no]
'''
sql = prql.to_sql(prql_query)
```

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.

```rust
#[pyfunction]
pub fn to_sql(query: &str) -> PyResult<String> {}
fn prql_python(_py: Python, m: &PyModule) -> PyResult<()> {}
```
