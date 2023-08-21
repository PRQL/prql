# Aggregation

A key feature of analytics is reducing many values down to some summary. This
act is called "aggregation" and always includes a function &mdash; for example,
`average` or `sum` &mdash; that reduces the table to a single row.

### `aggregate` transform

The `aggregate` transform takes a tuple to create one or more new columns that
"distill down" data from all the rows.

```
from invoices
aggregate { sum_of_orders = sum total }
```

The query above computes the sum of the `total` column of all rows of the
`invoices` table to produce a single value.

`aggregate` can produce multiple summaries at once when one or more aggregation
expressions are contained in a tuple. `aggregate` discards all columns that are
not present in the tuple.

```
from invoices
aggregate {
    num_orders = count this,
    sum_of_orders = sum total,
}
```

In the example above, the result is a single row with two columns. The `count`
function displays the number of rows in the table that was passed in; the `sum`
function adds up the values of the `total` column of all rows.

## Grouping

Suppose we want to produce summaries of invoices _for each city_ in the table.
We could create a query for each city, and aggregate its rows:

```
from albums
filter billing_city == "Oslo"
aggregate { sum_of_orders = sum total }
```

But we would need to do it for each city: `London`, `Frankfurt`, etc. Of course
this is repetitive (and boring) and error prone (because we would need to type
each `billing_city` by hand). Moreover, we would need to create a list of each
`billing_city` before we started.

### `group` transform

The `group` transform separates the table into groups (say, those having the
same city) using information that's already in the table. It then applies a
transform to each group, and combines the results back together:

```
from invoices
group billing_city (
    aggregate {
        num_orders = count this,
        sum_of_orders = sum total,
    }
)
```

Those familiar with SQL have probably noticed that we just decoupled aggregation
from grouping. These connected operations (in SQL) benefit from each being a
standalone function.

Firstly, each can have invariants that the query engine can leverage to produce
more efficient queries. Additionally, they can be used with other transform
functions, such as:

```
from invoices
group billing_city (
    take 2
)
```

This code collects the first two rows for each city's `group`.

### Move to Reference?

_The following items might be better moved to the Reference section_

The SQL needed to replicate this behavior might include window functions and
sub-queries. PRQL handles all the complexity. Some dialects (PostgreSQL, DuckDB,
Google BigQuery) have a special syntax to improve performance and reduce the
query complexity (for example ,`DISTINCT ON` if the query uses `take 1`). But
it's not a general solution for the case of `take 2`.
