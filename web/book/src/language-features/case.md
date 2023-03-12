# Case

```admonish note
`case` is currently experimental and may change behavior in the near future
```

PRQL uses `case` for both SQL's `CASE` and `IF` statements. Here's an example:

```prql_no_fmt
from employees
derive distance = case [
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
]
```

If no condition is met, the value takes a `null` value. To set a default, use a
`true` condition:

```prql_no_fmt
from employees
derive distance = case [
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
  true => "Unknown",
]
```
