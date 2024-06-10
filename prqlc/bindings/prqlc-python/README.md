# PRQL - Python Bindings

Python bindings for [PRQL](https://github.com/PRQL/prql), the Pipelined
Relational Query Language.

PRQL is a modern language for transforming data — a simple, powerful, pipelined
SQL replacement. Like SQL, it's readable, explicit and declarative. Unlike SQL,
it forms a logical pipeline of transformations, and supports abstractions such
as variables and functions. It can be used with any database that uses SQL,
since it compiles to SQL.

PRQL can be as simple as:

```
from tracks
filter artist == "Bob Marley"     # Each line transforms the previous result
aggregate {                       # `aggregate` reduces each column to a value
  plays    = sum plays,
  longest  = max length,
  shortest = min length,          # Trailing commas are allowed
}
```

## Installation

`pip install prqlc`

## Usage

Basic usage:

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

The following functions and classes are exposed:

```python
def compile(prql_query: str, options: Optional[CompileOptions] = None) -> str:
    """Compiles a PRQL query into SQL."""
    ...

def prql_to_pl(prql_query: str) -> str:
    """Converts a PRQL query to PL AST in JSON format."""
    ...

def pl_to_prql(pl_json: str) -> str:
    """Converts PL AST as a JSON string into a formatted PRQL string."""
    ...

def pl_to_rq(pl_json: str) -> str:
    """Resolves and lowers PL AST (JSON) into RQ AST (JSON)."""
    ...

def rq_to_sql(rq_json: str, options: Optional[CompileOptions] = None) -> str:
    """Converts RQ AST (JSON) into a SQL query."""
    ...

class CompileOptions:
    def __init__(
        self,
        *,
        format: bool = True,
        target: str = "sql.any",
        signature_comment: bool = True,
    ) -> None:
    """Compilation options for SQL backend of the compiler.

    Args:
        format (bool): Pass generated SQL string through a formatter that splits
            it into multiple lines and prettifies indentation and spacing.
            Defaults to True.
        target (str): Target dialect to compile to. Defaults to "sql.any", which
            uses the 'target' argument from the query header to determine the
            SQL dialect. Other targets are available by calling the `get_targets`
            function.
        signature_comment (bool): Emits the compiler signature as a comment after
            the generated SQL. Defaults to True.

    """
    ...

def get_targets() -> list[str]:
    """List available target dialects for compilation."""
    ...
```

### Debugging functions

The following functions are available within the `prqlc.debug` module. They are
for experimental purposes and may be unstable.

```python
def prql_lineage(prql_query: str) -> str:
    """Computes a column-level lineage graph from a PRQL query.

    Returns JSON-formatted string. See the docs for the `prqlc debug lineage`
    CLI command for more details.
    """
    ...

def pl_to_lineage(pl_json: str) -> str:
    """Computes a column-level lineage graph from PL AST (JSON)."""
    ...
```

## Notes

These bindings are in a crate named `prqlc-python` and published to a Python
package on PyPI named `prqlc`, available at <https://pypi.org/project/prqlc>.
This crate is not published to crates.io.

The package is consumed by [pyprql](https://github.com/prql/pyprql) &
[dbt-prql](https://github.com/prql/dbt-prql).

Relies on [pyo3](https://github.com/PyO3/pyo3) for all the magic.
