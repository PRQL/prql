# Sort

Order rows based on the values of one or more expressions (generally columns).

```prql no-eval
sort {(+|-) column}
```

## Parameters

- One expression or a tuple of expressions to sort by
- Each expression can be prefixed with:
  - `+`, for ascending order, the default
  - `-`, for descending order
- When using prefixes, even a single expression needs to be in a tuple or
  parentheses. (Otherwise, `sort -foo` is parsed as a subtraction between `sort`
  and `foo`.)

## Examples

```prql
from db.employees
sort age
```

```prql
from db.employees
sort {-age}
```

```prql
from db.employees
sort {age, -tenure, +salary}
```

We can also use expressions:

```prql
from db.employees
sort {s"substr({first_name}, 2, 5)"}
```

## Ordering guarantees

Ordering is persistent through a pipeline in PRQL. For example:

```prql
from db.employees
sort tenure
join db.locations (==employee_id)
```

Here, PRQL pushes the `sort` down the pipeline, compiling the `ORDER BY` to the
_end_ of the query. Consequently, most relation transforms retain the row order.

The explicit semantics are:

- `sort` introduces a new order,
- `group` resets the order,
- `join` retains the order of the left relation,
- database tables don't have a known order.

Comparatively, in SQL, relations possess no order, being orderable solely within
the context of the query result, `LIMIT` statement, or window function. The lack
of inherent order can result in an unexpected reshuffling of a previously
ordered relation from a `JOIN` or windowing operation.

```admonish info
To be precise — in PRQL, a relation is an _array of tuples_ and not a set or a bag.
The persistent nature of this order remains intact through sub-queries and intermediate
table definitions.
```

For instance, an SQL query such as:

```sql
WITH albums_sorted AS (
  SELECT *
  FROM albums
  ORDER BY title
)
SELECT *
FROM albums_sorted
JOIN artists USING (artist_id)
```

...doesn't guarantee any row order (indeed — even without the `JOIN`, the SQL
standard doesn't guarantee an order, although most implementations will respect
it).

<!-- We rolling this back. Waiting on the outcome of https://github.com/PRQL/prql/issues/2622 -->

<!-- ## Nulls

PRQL defaults to `NULLS LAST` when compiling to SQL. Because databases have
different defaults, the compiler emits this for all targets for which it's not a
default{{footnote: except for MSSQL, which doesn't support this}}.

The main benefit of this approach is that `take 42` will select non-null values
for both ascending and descending sorts, which is generally what is wanted.

There isn't currently a way to change this for a query, but if that would be
helpful, please raise an issue.

Note how DuckDB doesn't require a `NULLS LAST`, unlike the generic targets
above:

```prql
prql target:sql.duckdb

from db.artists
sort artist_id
take 42
```

```admonish info
Check out [DuckDB #7174](https://github.com/duckdb/duckdb/pull/7174) for a survey of various databases' implementations.
```
