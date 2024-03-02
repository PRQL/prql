# Variables — `let` & `into`

Variables assign a name — say `x` — to an expression, like in most programming
languages. The name can then be used in any expression, acting as a substitute
for the expression `x`.

Syntactically, variables can take 3 forms.

- `let` declares the name before the expression.

  ```prql no-eval
  let my_name = x
  ```

- `into` declares the name after the expression. This form is useful for quick
  pipeline splitting and conforms with the "flow from top to bottom" rule of
  pipelines.

  ```prql no-eval
  x
  into my_name
  ```

- The final expression of a pipeline defaults to taking the name `main`.

  ```prql no-eval
  from x
  ```

  ... is equivalent to:

  ```prql no-eval
  let main = x
  ```

When compiling to SQL, relational variables are compiled to Common Table
Expressions (or sub-queries in some cases).

```prql
let top_50 = (
  from employees
  sort salary
  take 50
  aggregate {total_salary = sum salary}
)

from top_50      # Starts a new pipeline
```

```prql
from employees
take 50
into first_50

from first_50
```

Variables can be assigned an s-string containing the whole SQL query
[s-string](../syntax/s-strings.md), enabling us to use features which PRQL
doesn't yet support.

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
