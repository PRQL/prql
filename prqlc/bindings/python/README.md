# prql-python

`prql-python` offers Rust bindings to the `prql-compiler` Rust library. It
exposes a Python method `compile(query: str) -> str`.

This is consumed by [pyprql](https://github.com/prql/pyprql) &
[dbt-prql](https://github.com/prql/dbt-prql).

The crate is not published to crates.io; only to PyPI at
https://pypi.org/project/prql-python/.

## Installation

`pip install prql-python`

## Usage

```python
import prql_python as prql

prql_query = """
    from employees
    join salaries (==emp_id)
    group {employees.dept_id, employees.gender} (
      aggregate {
        avg_salary = average salaries.salary
      }
    )
"""

options = prql.CompileOptions(
    format=True, signature_comment=True, target="sql.postgres"
)

sql = prql.compile(prql_query)
sql_postgres = prql.compile(prql_query, options)
```

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.
