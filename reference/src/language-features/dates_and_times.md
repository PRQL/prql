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

Dates are represented by `d{yyyy-mm-dd}` — a `d` followed by the
 date format.

```prql
from employees
derive age_at_year_end: (d2022-12-31 - dob)
```

## Times

Times are represented by `t{HH:mm:ss.SSS}`, with an optional suffix of timezone
— a `t` followed by the ISO8601 time format.

```prql
from orders
derive should_have_shipped_today: (order_time < t08:30)
```

## Timestamps

Timestamps are represented by `ts{yyyy-mm-ddTHH:mm:ss.SSS}`, with an optional suffix of timezone
— a `ts` followed by the ISO8601 time format, which uses `T` to separate date & time.

```prql
derive first_prql_commit: ts2020-01-01T13:19:55-0800
```

## Datetimes

Datetimes are supported by some databases (e.g. MySql, BigQuery) in addition to
timestamps.

Datetimes are represented by `dt{yyyy-mm-ddTHH:mm:ss.SSS}`, with an optional suffix of timezone
— a `dt` followed by the ISO8601 time format, which uses `T` to separate date & time.

```prql
derive pi_day: dt2017-03-14T15:09:26.535898
```

## Intervals

It's possible to represent intervals with a single expression:

```prql
from projects
derive first_check_in: start + 10days
```
