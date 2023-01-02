# Switch

```admonish note
`switch` is currently experimental and may change behavior in the near future
```

PRQL uses `switch` for both SQL's `CASE` and `IF` statements. Here's an example:

```prql
from employees
derive distance = switch [
  city == "Calgary" -> 0,
  city == "Edmonton" -> 300,
]
```

If no condition is met, the value takes a `null` value. To set a default, use a
`true` condition:

```prql
from employees
derive distance = switch [
  city == "Calgary" -> 0,
  city == "Edmonton" -> 300,
  true -> "Unknown",
]
```
