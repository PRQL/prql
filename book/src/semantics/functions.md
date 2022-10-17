# Functions

## Function call

The major distinction between PRQL and today's conventional programming
languages such as C or Python is the function call syntax.
It consists of the function name followed by arguments separated by whitespace.

```prql_no_test
function_name arg1 arg2 arg3
```

If one of the arguments is also a function call, it must be encased in parentheses,
so we know where arguments of inner function end and the arguments of outer function start.

```prql_no_test
outer_func arg_1 (inner_func arg_a, arg_b) arg_2
```

## Pipeline

There is a alternative way of calling functions: using a pipeline.
Regardless of whether the pipeline is delimited by pipe symbol `|` or a new line,
the pipeline is equivalent to applying each of functions as the last argument of the next function.

```prql_no_test
a | foo 3 | bar 'hello' 'world' | baz
```

... is equivalent to ...

```prql_no_test
baz (bar 'hello' 'world' (foo 3 a))
```

As you may have noticed, transforms are regular functions too!

```prql
from employees
filter age > 50
sort name
```

... is equivalent to ...

```prql
from employees | filter age > 50 | sort name
```

... is equivalent to ...

<!-- TODO: these should work! But they currently fail -->

```prql_no_test
filter age > 50 (from employees) | sort name
```

... which is the same as:

```prql_no_test
sort name (filter age > 50 (from (employees))
```

As you can see, the first example with pipeline notation is much easier to comprehend,
compared to the last one with the regular function call notation.
This is why it is recommended to use pipelines for nested function calls that are 3 or more levels deep.

## Currying and late binding

In PRQL, functions are first class citizens.
As cool as that sounds, we need simpler terms to explain it.
In essence in means that we can operate with functions are with any other value.

<!-- TODO -->
