# Case

Search for the first condition that evaluates to `true` and return its
associated value. If none of the conditions match, `null` is returned.

```prql
from employees
derive distance = case [
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
]
```

To set a default, a `true` condition can be used:

```prql
from employees
derive distance = case [
  city == "Calgary" => 0,
  city == "Edmonton" => 300,
  true => "Unknown",
]
```
