# Expressions and operators

PRQL allows _expressions_, like `2 + 3` or `((1 + x) * y)` made up of various
_operators_. In the example below, note the use of expressions to calculate the
alias `circumference` and in the `filter` transform.

```prql
from foo
select [
  circumference = diameter * 3.14159,
  color,
]
filter circumference > 10 && color != "red"
```

## Operator precedence

This table shows operator precedence. Use parentheses `()` to prioritize
operations and for function calls (see below.) _Question: Are parentheses
operators? Should they be in this table?_

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

|          Group | Operators         | Precedence | Associativity |
| -------------: | ----------------- | :--------: | :-----------: |
| identifier dot | `.`               |     1      |               |
|          unary | `- + ! ==`        |     2      |               |
|          range | `..`              |     3      |               |
|            mul | `* / %`           |     4      | left-to-right |
|            add | `+ -`             |     5      | left-to-right |
|        compare | `== != <= >= < >` |     6      | left-to-right |
|       coalesce | `??`              |     7      | left-to-right |
|            and | `&&`              |     8      | left-to-right |
|             or | <code>\|\|</code> |     9      | left-to-right |
|  function call |                   |     10     |               |
|   _parens (?)_ | `()`              |     ??     |   see below   |

## Parentheses

PRQL uses parentheses `()` for several purposes:

- Parentheses group operands to control the order of evaluation, for example:
  `((1 + x) * y)`

- Parentheses set off an [inner transform](./inner-transforms.md) for the
  `group ()` and `window ()` transforms. _Question: Are there any other
  transforms that use parentheses like this?_

- Parentheses set off a minus sign for a function argument, for example:
  `add (-1) (-3)`

- Parentheses set off nested function calls that contain a pipe, either the `|`
  symbol or a new line. “Nested” means within a transform; i.e. not just the
  main pipeline, for example: `(column-name | in 0..20)`

- Parentheses wrap a function call that is part of a larger expression, for
  example: `round 0 (sum distance)` _Question: Is it necessary to state "on the
  right-hand side of an assignment"? Are there counterexamples?_

  _Question: Why not require parentheses around function calls in a list? Why
  preserve this (confusing) special case?_

- Parentheses are not required for expressions that do not contain function
  calls, for example: `foo + bar`.

Here's a set of examples of these rules:

```prql no-fmt
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
derive distance_rounded_2_dp = (round 1+1 distance)
derive [
  # Requires parentheses, because it contains a pipe
  is_far = (distance | in 100..),
  # The left value of the range requires parentheses,
  # because of the minus sign
  is_negative = (distance | in (-100..0)),
  # ...this is equivalent
  is_negative = (distance | in (-100)..0),
  # Doesn't require parentheses, because it's in a list (confusing, see footnote)!
  average_distance = average distance,
]
# Requires parentheses because of the minus sign
sort (-distance)
# A list is fine too
sort [-distance]
```

```admonish note
We are seeking feedback about these rules to see if they can be more intuitive.
We're also planning to make the error messages much better,
so the compiler is there to help out.
```
