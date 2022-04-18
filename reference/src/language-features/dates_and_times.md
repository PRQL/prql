# Dates & Times

PRQL uses a pattern of prefacing strings with their type to represent dates.
This is less verbose than SQL's approach of `TIMESTAMP '2004-10-19 10:23:54'`
and more explicit than SQL's implicit option of just using a string `'2004-10-19
10:23:54'`.

> Currently PRQL passes strings straight through to the database, and so any
compatible format string will work, but we may refine this in the future to aid
in compatibility across databases. We'll always support the
[ISO8601](https://en.wikipedia.org/wiki/ISO_8601) format.

## Dates

Dates are represented by `D{yyyy-mm-dd}` — a `D` followed by the
date format.

```prql
from employees
derive age_at_year_end: (D2022-12-31 - dob)
```

## Times

Times are represented by `T{HH:mm:ss.SSS±Z}` with any parts not supplied being
rounded to zero, including the timezone, which is represented by `+HH:mm`,
`-HH:mm` or `Z`. This is consistent with the ISO8601 time format.

```prql
from orders
derive should_have_shipped_today: (order_time < t08:30)
```

## Timestamps

Timestamps are represented by `TS{yyyy-mm-ddTHH:mm:ss.SSS±Z}` /
`TS{date}T{time}`, with any time parts not supplied being rounded to zero,
including the timezone, which is represented by `+HH:mm`, `-HH:mm` or `Z`. This
is `TS` followed by the ISO8601 time format, which uses `T` to separate date &
time.

```prql
derive first_prql_commit: TS2020-01-01T13:19:55-0800
```

## Datetimes

Datetimes are supported by some databases (e.g. MySql, BigQuery) in addition to
timestamps.

Datetimes are represented in the same way as Timestamps, but with a `DT` prefix
rather than a `TS` prefix.

```prql
derive pi_day: DT2017-03-14T15:09:26.535898
```

## Intervals

It's possible to represent intervals with a single expression:

```prql
from projects
derive first_check_in: start + 10days
```
