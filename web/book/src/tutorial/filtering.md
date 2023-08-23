# Filtering rows

In previous page we learned how `select`, `derive`, and `join` change the
columns of a table.

Now we will look into how we can manipulate rows of a table using `filter` and
`take`.

### `filter` transform

The `filter` transform picks rows to pass through based on their values:

```
from invoices
filter billing_city == "Berlin"
```

The resulting table contains all the rows that came from Berlin.

PRQL converts the single `filter` transform to use the appropriate SQL `WHERE`
or `HAVING` command, depending on where it appears in the pipeline it is used.

### `take` transform

The `take` transform picks rows to pass through based on their position within
the table. The position (the set of rows) can be specified in two ways:

- a plain number `x`, which will pick the first `x` rows, or
- an inclusive range of rows `start..end`.

```
from invoices
take 4
```

```
from invoices
take 4..7
```

Of course, it is possible combine all these transforms into a single pipeline:

```
from invoices

# retain only rows for orders from Berlin
filter billing_city == "Berlin"

# skip first 10 rows and take the next 10
take 11..20

# take only first 3 rows of *that* result
take 3
```

We did something a bit odd at the end: first we took rows `11..20` and then took
the first 3 rows from that result. Although this could have been done in a
single transform `take 11..13`, this is a nice example of how PRQL allows fast
data exploration.

We have the freedom to stack transforms on top of each other, without worrying
about interactions of a new transform with the previous query. When PRQL
compiles these two statements/transforms (`take 11..20` and `take 3`) to SQL, it
produces the same result as `take 11..13`.
