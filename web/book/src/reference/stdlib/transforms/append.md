# Append

Concatenates two tables together.

Equivalent to `UNION ALL` in SQL. The number of rows is always the sum of the
number of rows from the two input tables. To replicate `UNION DISTINCT`, see
[set operations](#set-operations).

```prql
from employees_1
append employees_2
```

## Remove

> _experimental_

Removes rows that appear in another relation, like `EXCEPT ALL`. Duplicate rows
are removed one-for-one.

```prql
from employees_1
remove employees_2
```

## Intersection

> _experimental_

```prql
from employees_1
intersect employees_2
```

## Set operations

> _experimental_

To imitate set operations i.e. (`UNION`, `EXCEPT` and `INTERSECT`), you can use
the following functions:

```prql no-eval
let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))
let union = `default_db.bottom` top -> (top | append bottom | distinct)
let except = `default_db.bottom` top -> (top | distinct | remove bottom)
let intersect_distinct = `default_db.bottom` top -> (top | intersect bottom | distinct)
```

Don't mind the `default_db.` and `noop`, these are compiler implementation
detail for now.
