# Append

Concatenates two tables together.

```prql no-eval
append by:{position|name} rel
```

Equivalent to `UNION ALL` in SQL. The number of rows is always the sum of the
number of rows from the two input tables. The number of columns in each input
table must be the same, and the columns will align by position by default.

```prql
from employees_1
append employees_2
```

To replicate `UNION DISTINCT`, see [set operations](#set-operations).

Tables can also be combined by column name rather than column position by adding
the `by:name` argument to `append`. When appending by name, the number of
columns in each table does not need to be the same; columns present in one
relation but missing from the other will have NULL values added. This mode
currently only works if the set of columns on both sides is fully defined.

```prql
from employees_1
select {id, name, dob, zip}
append by:name (
  from employees_2
  select {id, name, email, zip}
)
```

> Support for generating dialect-specific `UNION ALL BY NAME` queries is
> pending.

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

Don't mind the `default_db.`; this is a compiler implementation detail for now.
