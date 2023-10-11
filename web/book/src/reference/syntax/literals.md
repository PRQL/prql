# Literals

A literal is a constant value expression, with special syntax rules for each
data type.

## Numbers

Number literals can contain number characters as well as a period, underscores
and char `e`.

If a number literal contains a dot or character `e`, it is treated as floating
point number (or just _float_), otherwise it is treated as integer number.

Character `e` denotes
["scientific notation"](https://en.wikipedia.org/wiki/Scientific_notation),
where the number after `e` is the exponent in 10-base.

Underscores are ignored, so they can be placed at arbitrary positions, but it is
advised to use them as thousand separators.

Integers can, alternatively, be expressed using hexadecimal, or binary
notation using these prefixes respectively: `0x` or `0b`.

```prql
from numbers
select {
    small = 1.000_000_1,
    big = 5_000_000,
    huge = 5e9,
    binary = 0x0011,
    hex = 0x80,
}
```

## Strings

PRQL supports string literals and several other formats of strings. See the
[Strings](./strings.md) page for more information.

## Booleans

Boolean values can be expressed with `true` or `false` keyword.

## Null

The null value can be expressed with `null` keyword. See also the discussion of
how [PRQL handles nulls](../spec/null.md).

## Date and time

Date and time literals are expressed with character `@`, followed by a string
that encodes the date & time.

```admonish note
PRQL's notation is designed to be less verbose than SQL's
`TIMESTAMP '2004-10-19 10:23:54'` and more explicit than SQL's implicit option
that just uses a string `'2004-10-19 10:23:54'`.
```

### Dates

Dates are represented by `@{yyyy-mm-dd}` — a `@` followed by the date format.

```prql
from employees
derive age_at_year_end = (@2022-12-31 - dob)
```

### Times

Times are represented by `@{HH:mm:ss.SSS±Z}` with any parts not supplied
defaulting to zero. This includes the timezone, which is represented by
`+HH:mm`, `-HH:mm` or `Z`. This is consistent with the ISO8601 time format.

```prql
from orders
derive should_have_shipped_today = (order_time < @08:30)
```

### Timestamps

Timestamps are represented by `@{yyyy-mm-ddTHH:mm:ss.SSS±Z}` / `@{date}T{time}`,
with any time parts not supplied being rounded to zero, including the timezone,
which is represented by `+HH:mm`, `-HH:mm` or `Z` (`:` is optional). This is `@`
followed by the ISO8601 datetime format, which uses `T` to separate date & time.

```prql
from commits
derive first_prql_commit = @2020-01-01T13:19:55-08:00
derive first_prql_commit_utc = @2020-01-02T21:19:55Z
```

### Durations

Durations are represented by `{N}{periods}`, such as `2years` or `10minutes`,
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

### Examples

Here's a larger list of date and time examples:

- `@20221231` is invalid — it must contain full punctuation (`-` and `:`),
- `@2022-12-31` is a date
- `@2022-12` or `@2022` are invalid — SQL can't express a month, only a date
- `@16:54:32.123456` is a time
- `@16:54:32`, `@16:54`, `@16` are all allowed, expressing `@16:54:32.000000`,
  `@16:54:00.000000`, `@16:00:00.000000` respectively
- `@2022-12-31T16:54:32.123456` is a timestamp without timezone
- `@2022-12-31T16:54:32.123456Z` is a timestamp in UTC
- `@2022-12-31T16:54+02` is timestamp in UTC+2
- `@2022-12-31T16:54+02:00` and `@2022-12-31T16:54+02` are datetimes in UTC+2
- `@16:54+02` is invalid — time is always local, so it cannot have a timezone
- `@2022-12-31+02` is invalid — date is always local, so it cannot have a
  timezone

```admonish note
Currently prql-compiler does not parse or validate any of the datetime strings
and will pass them to the database engine without adjustment. This might be
refined in the future to aid in compatibility across databases. We'll always
support the canonical [ISO8601](https://en.wikipedia.org/wiki/ISO_8601) format
described above.
```

### Roadmap

Datetimes (as a distinct datatype from the timestamps) are supported by some
databases (e.g. MySql, BigQuery). With the addition of type casts, these could
be represented by a timestamp cast to a datetime:

```prql no-eval
derive pi_day = @2017-03-14T15:09:26.535898<datetime>
```

These are some examples we can then add:

- `@2022-12-31T16:54<datetime>` is datetime without timezone
- `@2022-12-31<datetime>` is forbidden — datetime must specify time
- `@16:54<datetime>` is forbidden — datetime must specify date
