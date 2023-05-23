# Sort

Orders rows based on the values of one or more columns.

```prql no-eval
sort [{direction}{column}]
```

## Parameters

- One column or a list of columns to sort by
- Each column can be prefixed with:
  - `+`, for ascending order, the default
  - `-`, for descending order
- When using prefixes, even a single column needs to be in a list or
  parentheses. (Otherwise, `sort -foo` is parsed as a subtraction between `sort`
  and `foo`.)

## Examples

```prql
from employees
sort age
```

```prql
from employees
sort {-age}
```

```prql
from employees
sort {age, -tenure, +salary}
```

We can also use expressions:

```prql
from employees
sort {s"substr({first_name}, 2, 5)"}
```

## Ordering guarantees

In PRQL, a relation is an _array of tuples_ and not a set or a bag. The
difference is that array has inherent ordering. This makes the order of the
relation persist trough sub-queries and intermediate table definitions.

In SQL, on the other hand, relations do not have an order and can be ordered
only in the context of query result, `TAKE` clause or window functions. This has
a unexpected effect where a previously ordered relation would get reshuffled
because of following JOIN or windowing operations.

For example, this query:

```sql
SELECT * FROM (SELECT * FROM albums ORDER BY title) sub_query
```

... does not guarantee any row order, according to the SQL standard. Even though
most SQL engine implementations will return albums ordered by title, this order
may be destroyed by a subsequent JOIN or windowing operation.

In practice, ORDER BY clauses to be pushed down the pipeline until a TAKE clause
or end of the query:

```prql
from employees
sort tenure
join locations {==employee_id}
```

Observe how PRQL compiles the `ORDER BY` to the _end_ of the query.

Most relation transforms retain the row order, but there are a few exceptions:

- `sort` applies a new order (obviously),
- `group` resets the order,
- `join` retains the order of the left relation,
- database tables have unknown order.
