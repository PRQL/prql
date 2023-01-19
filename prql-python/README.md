# prql-python

`prql-python` offers rust bindings to the `prql-compiler` rust library. It
exposes a python method `compile(query: str) -> str`.

This is consumed by [PyPrql](https://github.com/prql/PyPrql) &
[dbt-prql](https://github.com/prql/dbt-prql).

The crate is not published to crates.io; only to PyPI.

## Installation

`pip install prql-python`

## Usage

```python
import prql_python as prql

prql_query = """
    from employees
    join salaries [==emp_id]
    group [employees.dept_id, employees.gender] (
      aggregate [
        avg_salary = average salaries.salary
      ]
    )
"""

sql = prql.compile(prql_query)
```

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.
