# Variables

We can define a relation — similar to a CTE in SQL — with two approaches — a
prefix `let` or a postfix `into.

## `let`

Here we assign a variable to `foo` with `let` by prefixing with `let foo =`:

```prql
let top_50 = (
  from employees
  sort salary
  take 50
  aggregate {total_salary = sum salary}
)

from top_50      # Starts a new pipeline
```

We can even place a whole CTE in an
[s-string](../language-features/s-strings.md), enabling us to use features which
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

## `into`

We can also assign a variable to `foo` by postfixing with `into foo`:

```prql
from employees
sort salary
take 50
aggregate {total_salary = sum salary}
into top_50

from top_50      # Starts a new pipeline
```

```admonish info
In PRQL variables are far less common than CTEs are in SQL, since a linear series
of CTEs can be represented with a single pipeline.
```

Currently defining variables is restricted to relations. We'd like to extend
this to expressions that evaluate to scalars.

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
