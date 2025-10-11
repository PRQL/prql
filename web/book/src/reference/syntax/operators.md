# Operators

Expressions can be composed from _function calls_ and _operations_, such as
`2 + 3` or `((1 + x) * -y)`. In the example below, note the use of expressions
to calculate the alias `circumference` and in the `filter` transform.

```prql
from foo
select {
  circumference = diameter * 3.14159,
  area = (diameter / 2) ** 2,
  color,
}
filter circumference > 10 && color != "red"
```

## Operator precedence

This table shows operator precedence. Use parentheses `()` to prioritize
operations and for function calls (see the discussion below.)

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

|          Group | Operators                   | Precedence | Associativity |
| -------------: | --------------------------- | :--------: | :-----------: |
|    parentheses | `()`                        |     0      |   see below   |
| identifier dot | `.`                         |     1      |               |
|          unary | `-` `+` `!` `==`            |     2      |               |
|          range | `..`                        |     3      |               |
|            pow | `**`                        |     4      | right-to-left |
|            mul | `*` `/` `//` `%`            |     5      | left-to-right |
|            add | `+` `-`                     |     6      | left-to-right |
|        compare | `==` `!=` `===` `!==` `<=` `>=` `<` `>` |     7      | left-to-right |
|       coalesce | `??`                        |     8      | left-to-right |
|            and | `&&`                        |     9      | left-to-right |
|             or | <code>\|\|</code>           |     10     | left-to-right |
|  function call |                             |     11     |               |

## Division and integer division

The `/` operator performs division that always returns a float value, while the
`//` operator does integer division (truncated division) that always returns an
integer value.

```prql
prql target:sql.sqlite

from [
  {a = 5, b = 2},
  {a = 5, b = -2},
]
select {
  div_out = a / b,
  int_div_out = a // b,
}
```

## Coalesce

We can coalesce values with an `??` operator. Coalescing takes either the first
value or, if that value is null, the second value.

```prql
from orders
derive amount ?? 0
```

## Regex expressions

```admonish note
This is currently experimental
```

To perform a case-sensitive regex search, use the `~=` operator. This generally
compiles to `REGEXP`, though differs by dialect. A regex search means that to
match an exact value, the start and end need to be anchored with `^foo$`.

```prql
from tracks
filter (name ~= "Love")
```

```prql
prql target:sql.duckdb

from artists
filter (name ~= "Love.*You")
```

```prql
prql target:sql.bigquery

from tracks
filter (name ~= "\\bLove\\b")
```

```prql
prql target:sql.postgres

from tracks
filter (name ~= "\\(I Can't Help\\) Falling")
```

```prql
prql target:sql.mysql

from tracks
filter (name ~= "With You")
```

```prql
prql target:sql.sqlite

from tracks
filter (name ~= "But Why Isn't Your Syntax More Similar\\?")
```

## Parentheses

PRQL uses parentheses `()` for several purposes:

- Parentheses group operands to control the order of evaluation, for example:
  `((1 + x) * y)`

- Parentheses delimit a minus sign of a function argument, for example:
  `add (-1) (-3)`

- Parentheses delimit nested function calls that contain a pipe, either the `|`
  symbol or a new line. “Nested” means within a transform; i.e. not just the
  main pipeline, for example: `(column-name | in 0..20)`

- Parentheses wrap a function call that is part of a larger expression, for
  example: `math.round 0 (sum distance)`

Parentheses are _not_ required for expressions that do not contain function
calls, for example: `foo + bar`.

Here's a set of examples of these rules:

```prql
from employees
# Requires parentheses, because it contains a pipe
derive is_proximate = (distance | in 0..20)
# Requires parentheses, because it's a function call
derive total_distance = (sum distance)
# `??` doesn't require parentheses, as it's not a function call
derive min_capped_distance = (min distance ?? 5)
# No parentheses needed, because no function call
derive travel_time = distance / 40
# No inner parentheses needed around `1+1` because no function call
derive distance_rounded_2_dp = (math.round 1+1 distance)
derive {
  # Requires parentheses, because it contains a pipe
  is_far = (distance | in 100..),
  # The left value of the range requires parentheses,
  # because of the minus sign
  is_negative = (distance | in (-100..0)),
  # ...this is equivalent
  is_negative = (distance | in (-100)..0),
  # _Technically_, this doesn't require parentheses, because it's
  # the RHS of an assignment in a tuple
  # (this is especially confusing)
  average_distance = average distance,
}
# Requires parentheses because of the minus sign
sort (-distance)
# A tuple is fine too
sort {-distance}
```

For example, the snippet below produces an error because the `sum` function call
is not in a tuple.

```prql error no-fmt
from employees
derive total_distance = sum distance
```

...while with parentheses, it works at expected:

```prql
from employees
derive other_distance = (sum distance)
```

```admonish note
We're continuing to think whether these rules can be more intuitive.
We're also planning to make the error messages much better,
so the compiler can help out.
```

## Wrapping lines

Line breaks in PRQL have semantic meaning, so to wrap a single logical line into
multiple physical lines, we can use `\` at the beginning of subsequent physical
lines:

```prql
from artists
select is_europe =
\ country == "DE"
\ || country == "FR"
\ || country == "ES"
```

Wrapping will "jump over" empty lines or lines with comments. For example, the
`select` here is only one logical line:

```prql
from tracks
# This would be a really long line without being able to split it:
select listening_time_years = (spotify_plays + apple_music_plays + pandora_plays)
# We can toggle between lines when developing:
# \ * length_seconds
\ * length_s
#   min  hour day  year
\ / 60 / 60 / 24 / 365
```

```admonish info
Note that PRQL differs from most languages, which use a `\` at the _end_ of the
preceding line. Because PRQL aims to be friendly for data exploration, we want
to make it possible to comment out any line, including the final line, without
breaking the query. This requires all lines after the first to be structured similarly,
and for the character to be at the start of each following line.
```

See [Pipes](./pipes.md) for more details on line breaks.
