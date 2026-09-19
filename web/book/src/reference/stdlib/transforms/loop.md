# Loop

> _Experimental_

```prql no-eval
loop (step_pipeline)
```

Iteratively applies `step_pipeline` to the relation piped into `loop`, until the
pipeline returns an empty table. Returns a relation that contains rows of the
initial relation and all intermediate relations.

This behavior could be expressed with following pseudo-code:

```python
def loop(step_pipeline, initial):
    result = []
    current = initial
    while current is not empty:
        result = append(result, current)
        current = step_pipeline(current)

    return result
```

## Examples

```prql
from [{n = 1}]
loop (
    filter n<4
    select n = n+1
)

# returns [1, 2, 3, 4]
```

<!-- prettier-ignore -->
> [!NOTE]
> The behavior of `WITH RECURSIVE` may depend on the database
> configuration in MySQL. The compiler assumes the behavior described by the
> [Postgres documentation](https://www.postgresql.org/docs/15/queries-with.html#QUERIES-WITH-RECURSIVE)
> and will not produce correct results for
> [alternative configurations of MySQL](https://dev.mysql.com/doc/refman/8.0/en/with.html#common-table-expressions-recursive).

<!-- prettier-ignore -->
> [!NOTE]
> Currently, `loop` may produce references to the recursive CTE in
> sub-queries, which is not supported by some database engines, e.g. SQLite. For
> now, we suggest step functions are kept simple enough to fit into a single
> SELECT statement.
