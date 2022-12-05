# Window

Applies a pipeline to segments of rows, producing one output value for every
input value.

```prql_no_test
window rows:{range} range:{range} expanding:false rolling:0 {pipeline}
```

For each row, the segment over which the pipeline is applied is determined by
one of:

- `rows`, which takes a range of rows relative to the current row position.
  - `0` references the current row.
- `range`, which takes a range of values relative to current row value.

The bounds of the range are inclusive. If a bound is omitted, the segment will
extend until the edge of the table or group.

<!-- TODO: rows vs range example, with visualization -->

For ease of use, there are two flags that override `rows` or `range`:

- `expanding:true` is an alias for `rows:..0`. A sum using this window is also
  known as "cumulative sum".
- `rolling:n` is an alias for `row:(-n+1)..0`, where `n` is an integer. This
  will include `n` last values, including current row. An average using this
  window is also knows as a Simple Moving Average.

Some examples:

| Expression       | Meaning                                                            |
| ---------------- | ------------------------------------------------------------------ |
| `rows:0..2`      | current row plus two following                                     |
| `rows:-2..0`     | two preceding rows plus current row                                |
| `rolling:3`      | (same as previous)                                                 |
| `rows:-2..4`     | two preceding rows plus current row plus four following rows       |
| `rows:..0`       | all rows from the start of the table up to & including current row |
| `expanding:true` | (same as previous)                                                 |
| `rows:0..`       | current row and all following rows until the end of the table      |
| `rows:..`        | all rows, which same as not having window at all                   |

## Example

```prql
from employees
group employee_id (
  sort month
  window rolling:12 (
    derive [trail_12_m_comp = sum paycheck]
  )
)
```

```prql
from orders
sort day
window rows:-3..3 (
  derive [centered_weekly_average = average value]
)
group [order_month] (
  sort day
  window expanding:true (
    derive [monthly_running_total = sum value]
  )
)
```

## Windowing by default

If you use window functions without `window` transform, they will be applied to the whole table. Unlike in SQL, they will remain window functions and will not trigger aggregation.

```prql
from employees
sort age
derive rnk = rank
```

You can also only apply `group`:

```prql
from employees
group department (
  sort age
  derive rnk = rank
)
```

## Window functions as first class citizens

There is no limitaions where windowed expressions can be used:

```prql
from employees
filter salary < (average salary)
```
