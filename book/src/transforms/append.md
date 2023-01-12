# Append & set operators

```admonish note
`append`, `union`, `difference` and `intersection` are currently experimental
and may have bugs; please report any as GitHub Issues.
```

## Append

`append` concatenates two tables together, like `UNION ALL` in SQL. The number
of rows is always the sum of the number of rows from the two input tables.

```prql
from employees_1
append employees_2
```

## Union

`union` takes the union of rows, where duplicates are discarded (using the
definition of `union` from set logic), like `UNION DISTINCT` in SQL. If all rows
are different between the tables, this is synonymous with `append`; if there are
duplicate rows it will produce fewer rows.

```prql
from employees_1
union employees_2
```

## Intersection

```prql
from employees_1
select [first_name, last_name]
intersection (
    from employees_2
    select [first_name, last_name]
)
```

## Difference

```prql
from employees_1
select [first_name, last_name]
difference (
    from employees_2
    select [first_name, last_name]
)
```

```admonish note
Currently, unknown columns (wildcards) may not work with set operators or may
even produce invalid SQL.
```
