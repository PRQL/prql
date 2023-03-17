# Functions

<!--
TODOs:
- Examples are a bit artificial — the interp is just "divide by 100" in one case!  -->

Functions are a fundamental abstraction in PRQL — they allow us to run code in
many places that we've written once. This reduces the number of errors in our
code, makes our code more readable, and simplifies making changes.

Functions have two types of parameters:

1. Positional parameters, which require an argument.
2. Named parameters, which optionally take an argument, otherwise using their
   default value.

So this function is named `fahrenheit_to_celsius` and has one parameter `temp`:

```prql_no_fmt
func fahrenheit_to_celsius temp -> (temp - 32) / 1.8

from cities
derive temp_c = (fahrenheit_to_celsius temp_f)
```

This function is named `interp`, and has two positional parameters named `high`
and `x`, and one named parameter named `low` which takes a default argument of
`0`. It calculates the proportion of the distance that `x` is between `low` and
`high`.

```prql
func interp low:0 high x -> (x - low) / (high - low)

from students
derive [
  sat_proportion_1 = (interp 1600 sat_score),
  sat_proportion_2 = (interp low:0 1600 sat_score),
]
```

## Piping

Consistent with the principles of PRQL, it's possible to pipe values into
functions, which makes composing many functions more readable. When piping a
value into a function, the value is passed as an argument to the final
positional parameter of the function. Here's the same result as the examples
above with an alternative construction:

```prql
func interp low:0 high x -> (x - low) / (high - low)

from students
derive [
  sat_proportion_1 = (sat_score | interp 1600),
  sat_proportion_2 = (sat_score | interp low:0 1600),
]
```

and

```prql
func fahrenheit_to_celsius temp -> (temp - 32) / 1.8

from cities
derive temp_c = (temp_f | fahrenheit_to_celsius)
```

We can combine a chain of functions, which makes logic more readable:

```prql
func fahrenheit_to_celsius temp -> (temp - 32) / 1.8
func interp low:0 high x -> (x - low) / (high - low)

from kettles
derive boiling_proportion = (temp_c | fahrenheit_to_celsius | interp 100)
```

## Scope

### Late binding

Functions can binding to any variables in scope when the function is executed.
For example, here `cost_total` refers to the column that's introduced in the
`from`.

```prql
func cost_share cost -> cost / cost_total

from costs
select [materials, labor, overhead, cost_total]
derive [
  materials_share = (cost_share materials),
  labor_share = (cost_share labor),
  overhead_share = (cost_share overhead),
]
```
