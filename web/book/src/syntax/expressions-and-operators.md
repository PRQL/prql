# Expressions and operators

PRQL allows _expressions_, like `2 + 3` or `((1 + x) * y)` made up of various
_operators_. In the example below, note the use of expressions to calculate the
alias `circumference` and in the `filter` transform.

```prql
from foo
select {
  circumference = diameter * 3.14159,
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
|            mul | `*` `/` `//` `%`            |     4      | left-to-right |
|            add | `+` `-`                     |     5      | left-to-right |
|        compare | `==` `!=` `<=` `>=` `<` `>` |     6      | left-to-right |
|       coalesce | `??`                        |     7      | left-to-right |
|            and | `&&`                        |     8      | left-to-right |
|             or | <code>\|\|</code>           |     9      | left-to-right |
|  function call |                             |     10     |               |

## Parentheses

PRQL uses parentheses `()` for several purposes:

- Parentheses group operands to control the order of evaluation, for example:
  `((1 + x) * y)`

- Parentheses delimit an [inner transform](./inner-transforms.md) for the
  `group ()` and `window ()` transforms.

- Parentheses delimit a minus sign of a function argument, for example:
  `add (-1) (-3)`

- Parentheses delimit nested function calls that contain a pipe, either the `|`
  symbol or a new line. “Nested” means within a transform; i.e. not just the
  main pipeline, for example: `(column-name | in 0..20)`

- Parentheses wrap a function call that is part of a larger expression, for
  example: `round 0 (sum distance)`

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
derive distance_rounded_2_dp = (round 1+1 distance)
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
