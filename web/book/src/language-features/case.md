# Case

```admonish note
`case` is currently experimental and may change behavior in the near future
```

```admonish info
`case` was previously (PRQL 0.4 to 0.5) called `switch` and renamed to `case` in PRQL 0.6.0.
```

PRQL uses `case` for both SQL's `CASE` and `IF` statements. Here's an example:

```prql
from employees
derive distance = case {
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
}
```

If no condition is met, the value takes a `null` value. To set a default, use a
`true` condition:

```prql
from employees
derive distance = case {
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
  true => "Unknown",
}
```
