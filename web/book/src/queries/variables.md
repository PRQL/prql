# Variables and `let`

`let` assigns a scalar value, a function, an array, or a pipeline to a named
variable.

## Scalar value

Define a constant that might be used multiple times in a query:

```
let pi = 3.14159
```

## Function

Define a function (full description in [Functions](./functions.md)):

```
let fahrenheit_to_celsius = temp -> (temp - 32) / 1.8
```

## Array

Define a relation using the array (`[...]`) notation:
```
let table = [{a=5, b=false}, {a=6, b=true}]
```

## Pipeline

Define a relation — similar to a CTE in SQL — with two approaches — a prefix
`let` or a postfix `into`. This example assigns a relation (defined by a
pipeline of commands) to the variable `top_50`:

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

```admonish info
**Why introduce `into`?** It seems that `let = ...` completely covers the use case without introducing a new language feature.
Does `into` work with scalars and functions?
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
**Is this advice still relevant?**
In PRQL variables are far less common than CTEs are in SQL, since a linear series
of CTEs can be represented with a single pipeline.
```

```admonish info
**NO LONGER TRUE?**
Currently defining variables is restricted to relations. We'd like to extend
this to expressions that evaluate to scalars.
```

```admonish info
**This was commented out - is this still relevant?**
... like recursive queries:

TODO: get this example to work by removing the restriction to start with SELECT

Example from https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#recursive_keyword

table recursive_example = (s"""
  WITH RECURSIVE
    T1 AS ( (SELECT 1 AS n) UNION ALL (SELECT n + 1 AS n FROM T1 WHERE n < 3) )
  SELECT n FROM T1
""")

from recursive_example

```
