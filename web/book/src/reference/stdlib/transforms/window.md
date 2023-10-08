# Window

Applies a pipeline to segments of rows, producing one output value for every
input value.

```prql no-eval
window rows:(range) range:(range) expanding:false rolling:0 (pipeline)
```

For each row, the segment over which the pipeline is applied is determined by
one of:

- `rows`, which takes a range of rows relative to the current row position.
  - `0` references the current row.
- `range`, which takes a range of values relative to current row value.

The bounds of the range are inclusive. If a bound is omitted, the segment will
extend until the edge of the table or group.

For ease of use, there are two flags that override `rows` or `range`:

- `expanding:true` is an alias for `rows:..0`. A sum using this window is also
  known as "cumulative sum".
- `rolling:n` is an alias for `rows:(-n+1)..0`, where `n` is an integer. This
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
    derive {trail_12_m_comp = sum paycheck}
  )
)
```

```prql
from orders
sort day
window rows:-3..3 (
  derive {centered_weekly_average = average value}
)
group {order_month} (
  sort day
  window expanding:true (
    derive {monthly_running_total = sum value}
  )
)
```

Rows vs Range:

```prql no-eval
from [
  {time_id=1, value=15},
  {time_id=2, value=11},
  {time_id=3, value=16},
  {time_id=4, value=9},
  {time_id=7, value=20},
  {time_id=8, value=22},
]
window rows:-2..0 (
  sort time_id
  derive {sma3rows = average value}
)
window range:-2..0 (
  sort time_id
  derive {sma3range = average value}
)
```

| time_id | value | sma3rows | sma3range |
| ------- | ----- | -------- | --------- |
| 1       | 15    | 15       | 15        |
| 2       | 11    | 13       | 13        |
| 3       | 16    | 14       | 14        |
| 4       | 9     | 12       | 12        |
| 7       | 20    | 15       | 20        |
| 8       | 22    | 17       | 21        |

We can see that rows having `time_id` of 5 and 6 are missing in example data; we
can say there are gaps in our time series data.

When computing SMA 3 for the fifth row (`time_id==7`) then:

- "rows" will compute average on 3 rows (`time_id` in `3, 4, 7`)
- "range" will compute average on single row only (`time_id==7`)

When computing SMA 3 for the sixth row (`time_id==8`) then:

- "rows" will compute average on 3 rows (`time_id` in `4, 7, 8`)
- "range" will compute average on 2 rows (`time_id` in `7, 8`)

We can observe that "rows" ignores the content of the `time_id`, only uses its
order; we can say its window operates on physical rows. On the other hand
"range" looks at the content of the `time_id` and based on the content decides
how many rows fits into window; we can say window operates on logical rows.

## Windowing by default

If you use window functions without `window` transform, they will be applied to
the whole table. Unlike in SQL, they will remain window functions and will not
trigger aggregation.

```prql
from employees
sort age
derive {rnk = rank age}
```

You can also only apply `group`:

```prql
from employees
group department (
  sort age
  derive {rnk = rank age}
)
```

## Window functions as first class citizens

There are no limitations on where windowed expressions can be used:

```prql
from employees
filter salary < (average salary)
```
