# Tables

We can define a temporary table — similar to a CTE in SQL — with `let`:

```prql
let top_50 = (
  from employees
  sort salary
  take 50
  aggregate [total_salary = sum salary]
)

from top_50      # Starts a new pipeline
```

We can even place a whole CTE in an s-string, enabling us to use features which
PRQL doesn't yet support.

```prql
let grouping = s"""
  SELECT SUM(a)
  FROM tbl
  GROUP BY
    GROUPING SETS
    ((b, c, d), (d), (b, d))
"""

from grouping
```

```admonish info
In PRQL `table`s are far less common than CTEs are in SQL, since a linear series
of CTEs can be represented with a single pipeline.
```

<!--
, like recursive queries:

TODO: get this example to work by removing the restriction to start with SELECT

Example from https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#recursive_keyword

table recursive_example = (s"""
  WITH RECURSIVE
    T1 AS ( (SELECT 1 AS n) UNION ALL (SELECT n + 1 AS n FROM T1 WHERE n < 3) )
  SELECT n FROM T1
""")

from recursive_example

-->
