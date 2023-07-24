# Variables

Variables assign a name to an expression (let's call it `x`). The name can then
be used in any expression, acting as a substitute for the expression `x`.

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

- implicit name, does not declare a name at all, but uses name `main` as the
  default. This is practical, as `main` is the variable that is compiled as the
  main relational query by default.

  ```prql no-eval
  x
  ```

  ... is equivalent to:

  ```
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

```prqls
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
