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

This table gives formal description of operator precedence. Because function
calls have the lowest precedence, nested function calls or arguments that start
or end with an operator require parentheses.

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

| Group          | Operators         | Precedence | Associativity |
| -------------- | ----------------- | ---------- | ------------- |
| identifier dot | `.`               | 1          |               |
| unary          | `- + ! ==`        | 2          |               |
| range          | `..`              | 3          |               |
| mul            | `* / %`           | 4          | left-to-right |
| add            | `+ -`             | 5          | left-to-right |
| compare        | `== != <= >= < >` | 6          | left-to-right |
| coalesce       | `??`              | 7          | left-to-right |
| and            | `&&`              | 8          | left-to-right |
| or             | <code>\|\|</code> | 9          | left-to-right |
| function call  |                   | 10         |               |
