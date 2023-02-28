# Dates & times

PRQL uses `@` followed by a string to represent dates & times. This is less
verbose than SQL's approach of `TIMESTAMP '2004-10-19 10:23:54'` and more
explicit than SQL's implicit option of just using a string
`'2004-10-19 10:23:54'`.

```admonish note
Currently PRQL passes strings which can be compiled straight through to the
database, and so many compatible formats string may work, but we may refine this
in the future to aid in compatibility across databases. We'll always support the
canonical [ISO8601](https://en.wikipedia.org/wiki/ISO_8601) format described below.
```

## Dates

Dates are represented by `@{yyyy-mm-dd}` — a `@` followed by the date format.

```prql
from employees
derive age_at_year_end = (@2022-12-31 - dob)
```

## Times

Times are represented by `@{HH:mm:ss.SSS±Z}` with any parts not supplied being
rounded to zero, including the timezone, which is represented by `+HH:mm`,
`-HH:mm` or `Z`. This is consistent with the ISO8601 time format.

```prql
from orders
derive should_have_shipped_today = (order_time < @08:30)
```

## Timestamps

Timestamps are represented by `@{yyyy-mm-ddTHH:mm:ss.SSS±Z}` / `@{date}T{time}`,
with any time parts not supplied being rounded to zero, including the timezone,
which is represented by `+HH:mm`, `-HH:mm` or `Z`. This is `@` followed by the
ISO8601 datetime format, which uses `T` to separate date & time.

```prql
from commits
derive first_prql_commit = @2020-01-01T13:19:55-08:00
```

## Intervals

Intervals are represented by `{N}{periods}`, such as `2years` or `10minutes`,
without a space.

```admonish note
These aren't the same as ISO8601, because we evaluated `P3Y6M4DT12H30M5S` to
be difficult to understand, but we could support a simplified form if there's
demand for it. We don't currently support compound expressions, for example
`2years10months`, but most DBs will allow `2years + 10months`. Please raise an
issue if this is inconvenient.
```

```prql
from projects
derive first_check_in = start + 10days
```

## Examples

Here's a fuller list of examples:

- `@20221231` is forbidden — it must contain full punctuation (`-` and `:`),
- `@2022-12-31` is a date
- `@2022-12` or `@2022` are forbidden — SQL can't express a month, only a date
- `@16:54:32.123456` is a time
- `@16:54:32`, `@16:54`, `@16` are all allowed, expressing `@16:54:32.000000`,
  `@16:54:00.000000`, `@16:00:00.000000` respectively
- `@2022-12-31T16:54:32.123456` is a timestamp without timezone
- `@2022-12-31T16:54:32.123456Z` is a timestamp in UTC
- `@2022-12-31T16:54+02` is timestamp in UTC+2
- `@2022-12-31T16:54+02:00` and `@2022-12-31T16:54+02` are datetimes in UTC+2
- `@16:54+02` is forbidden — time is always local, so it cannot have a timezone
- `@2022-12-31+02` is forbidden — date is always local, so it cannot have a
  timezone

## Roadmap

### Datetimes

Datetimes are supported by some databases (e.g. MySql, BigQuery) in addition to
timestamps. When we have type annotations, these will be represented by a
timestamp annotated as a datetime:

```prql_no_test
derive pi_day = @2017-03-14T15:09:26.535898<datetime>
```

These are some examples we can then add:

- `@2022-12-31T16:54<datetime>` is datetime without timezone
- `@2022-12-31<datetime>` is forbidden — datetime must specify time
- `@16:54<datetime>` is forbidden — datetime must specify date
