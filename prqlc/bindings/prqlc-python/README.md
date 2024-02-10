# Python bindings to `prqlc`

The `prqlc-python` crate offer Rust bindings to the `prqlc` Rust library,
published to a python package named `prqlc`.

The main entry point is a Python method `prqlc.compile(query: str) -> str`.

The package is consumed by [pyprql](https://github.com/prql/pyprql) &
[dbt-prql](https://github.com/prql/dbt-prql).

<!-- TODO: change -->
<!-- <https://pypi.org/project/prqlc/>. -->

The crate is not published to crates.io; only to PyPI at
<https://pypi.org/project/prql-python/>.

## Installation

`pip install prqlc`

## Usage

```python
import prqlc

prql_query = """
    from employees
    join salaries (==emp_id)
    group {employees.dept_id, employees.gender} (
      aggregate {
        avg_salary = average salaries.salary
      }
    )
"""

options = prqlc.CompileOptions(
    format=True, signature_comment=True, target="sql.postgres"
)

sql = prqlc.compile(prql_query)
sql_postgres = prqlc.compile(prql_query, options)
```

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.
