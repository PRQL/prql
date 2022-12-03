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

So this function is named `celsius_of_fahrenheit` and has one parameter `temp`:

```prql
func celsius_of_fahrenheit temp -> (temp - 32) / 1.8

from cities
derive temp_c = (celsius_of_fahrenheit temp_f)
```

This function is named `interp`, and has two positional parameters named
`higher` and `x`, and one named parameter named `lower` which takes a default
argument of `0`. It calculates the proportion of the distance that `x` is
between `lower` and `higher`.

```prql
func interp lower:0 higher x -> (x - lower) / (higher - lower)

from students
derive [
  sat_proportion_1 = (interp 1600 sat_score),
  sat_proportion_2 = (interp lower:0 1600 sat_score),
]
```

## Piping

Consistent with the principles of PRQL, it's possible to pipe values into
functions, which makes composing many functions more readable. When piping a
value into a function, the value is passed as an argument to the final
positional parameter of the function. Here's the same result as the examples
above with an alternative construction:

```prql
func interp lower:0 higher x -> (x - lower) / (higher - lower)

from students
derive [
  sat_proportion_1 = (sat_score | interp 1600),
  sat_proportion_2 = (sat_score | interp lower:0 1600),
]
```

and

```prql
func celsius_of_fahrenheit temp -> (temp - 32) / 1.8

from cities
derive temp_c = (temp_f | celsius_of_fahrenheit)
```

We can combine a chain of functions, which makes logic more readable:

```prql
func celsius_of_fahrenheit temp -> (temp - 32) / 1.8
func interp lower:0 higher x -> (x - lower) / (higher - lower)

from kettles
derive boiling_proportion = (temp_c | celsius_of_fahrenheit | interp 100)
```

## Roadmap

### Late binding

Currently, functions require a binding to variables in scope; they can't
late-bind to column names; so for example:

```prql_no_test
func return price -> (price - dividend) / price_yesterday
```

...isn't yet a valid function, and instead would needs to be:

```prql_no_test
func return price dividend price_yesterday ->  (price - dividend) / (price_yesterday)
```

(which makes functions in this case not useful)
