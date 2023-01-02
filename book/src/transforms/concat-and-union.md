# Concat & Union

```admonish note
`concot` & `union` are currently experimental and may have bugs; please
report any as GitHub Issues.
```

## Concat

`concat` concatenates two tables together, like `UNION ALL` in SQL. The number
of rows is always the sum of the number of rows from the two input tables.

```prql
from employees_1
concat employees_2
```

## Union

`union` takes the union of rows, where duplicates are discarded (using the
definition of `union` from set logic), like `UNION DISTINCT` in SQL. If all rows
are different between the tables, this is synonymous with `concat`; if there are
duplicate rows it will produce fewer rows.

```prql
from employees_1
union employees_2
```

## Roadmap

We'd also like to implement the set operations of `intersect` and `difference`.
