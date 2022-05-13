# Window

Applies a pipeline to overlapping segments of rows.

```prql_no_test
window rows:.. range:.. expanding:false rolling:0 {pipeline}
```

For each row in result, its input segment is determined either by:

- `rows`, which takes a range of rows relative to current row,
- `range`, which takes a range of values relative to current row value.

Start of the range is inclusive, end of range is exclusive. Index 0 references 
current row. If a bound is omitted, segment will extend until the end of the table (or group).

For example:
- `rows:0..3`   means current row plus two following,
- `rows:-3..0`  means two preceding rows plus current row,
- `rows:-3..5`  means two preceding rows plus current row plus four following rows,
- `rows:..1`    means all rows from the start of the table to and including current row,
- `rows:0..`    means current row and all following rows until the end of the table,
- `rows:..`     means all rows, which same as not having window at all.

> Note: currently, negative integer literals (`-3`) are not implemented.

<!-- TODO: rows vs range example, with visualization -->

For ease of use, there are two flags that override `rows` or `range`:

- `expanding:true` is an alias for `range:..1`. Sum using this window is also known as "cumulative sum".
- `rolling:x` is an alias for `row:(-x+1)..1`, where `x` is an integer. This will include `x` last values, including current row.

> Note: this row and range notation makes it easy to determine total number of rows included: `end - start`. In contrast, SQL does not make this easy with
> ```sql
> BETWEEN 2 PRECEDING -- will include 3 rows
> ```
> ```sql
> BETWEEN 2 PRECEDING AND 1 FOLLOWING -- will include 4 rows
> ```

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
