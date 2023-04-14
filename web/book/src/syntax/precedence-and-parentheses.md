# Precedence and Parentheses

Parentheses — `()` — are used
to give _precedence_ to inner expressions.

```admonish note
We realize some of the finer points here are not intuitive. We are considering approaches to
make this more intuitive — even at the cost of requiring more syntax in some
circumstances. And we're planning to make the error messages much better,
so the compiler is there to help out.
```

Parentheses are required around:

- Any nested function call containing a pipe, either the `|` symbol or a new
  line. "Nested" means within a transform; i.e. not just the main pipeline.
- Any function call that isn't a single item in a list or a pipeline, like
  `sum distance` in <br>
  `round 0 (sum distance)`{{footnote: or, technically, it's on the right
  side of an assignment in a list...}}.
- A minus sign in a function argument, like in `add (-1) (-3)`
- [Inner transforms](#inner-transforms) for `group`, `window`, and other
  transforms.

Parentheses are not required around expressions which use operators but no
function call, like `foo + bar`.

Here's a full rundown of times this applier:

```prql no-fmt
from employees
# Requires parentheses, because it's contains a pipe
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

This doesn't work, for example (though it should provide a much better error
message):

```prql error
from employees
derive total_distance = sum distance
```
