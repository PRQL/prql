# Window

Applies a pipeline to overlapping segments of rows.

```prql_no_test
window rows:.. range:.. expanding:false rolling:0 {pipeline}
```

For each row in result, its input segment is determined either by:

- `rows`, which takes a range of rows relative to current row,
- `range`, which takes a range of values relative to current row value.

Bounds of the range are inclusive. Index 0 references current row. If a
bound is omitted, segment will extend until the edge of the table (or group).

For example:

- `rows:0..2`   means current row plus two following,
- `rows:-2..0`  means two preceding rows plus current row,
- `rows:-2..4`  means two preceding rows plus current row plus four following rows,
- `rows:..0`    means all rows from the start of the table to and including current row,
- `rows:0..`    means current row and all following rows until the end of the table,
- `rows:..`     means all rows, which same as not having window at all.

> Note: currently, negative integer literals (`-3`) are not implemented.

<!-- TODO: rows vs range example, with visualization -->

For ease of use, there are two flags that override `rows` or `range`:

- `expanding:true` is an alias for `rows:..0`. Sum using this window is also known as "cumulative sum".
- `rolling:n` is an alias for `row:(-n+1)..0`, where `n` is an integer. This will include `n` last values, including current row.

## Example

```prql
from orders
sort day
window rolling:3 (
  derive [total_last_3_days = sum price]
)
group [order_month] (
  sort day
  window expanding:true (
    derive [monthly_running_total = sum price]
  )
)
```
