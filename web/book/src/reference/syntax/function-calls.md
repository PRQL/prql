# Function calls

## Simple

A distinction between PRQL and most other programming languages is the function
call syntax. It consists of the function name followed by arguments separated by
whitespace.

```prql no-eval
function_name arg1 arg2 arg3
```

If one of the arguments is also a function call, it must be encased in
parentheses, so we know where arguments of inner function end and the arguments
of outer function start.

```prql no-eval
outer_func arg_1 (inner_func arg_a, arg_b) arg_2
```

The function name must refer to a function variable, which has either
[been declared](../declarations/functions.md) in the
[standard library](../stdlib/) or some other module.

Function calls can also specify named parameters using `:` notation:

```prql no-eval
function_name arg1 named_param:arg2 arg3
```

## Pipeline

There is a alternative way of calling functions: using a pipeline. Regardless of
whether the pipeline is delimited by pipe symbol `|` or a new line, the pipeline
is equivalent to applying each of functions as the last argument of the next
function.

```prql no-eval
a | foo 3 | bar 'hello' 'world' | baz
```

... is equivalent to ...

```prql no-eval
baz (bar 'hello' 'world' (foo 3 a))
```

<!--
TODO: this should be a part of the tutorial


As you may have noticed, transforms are regular functions too!

```prql
from.employees
filter age > 50
sort name
```

... is equivalent to ...

```prql
from.employees | filter age > 50 | sort name
```

... is equivalent to ...

```prql
filter age > 50 (from.employees) | sort name
```

... is equivalent to ...

```prql
sort name (filter age > 50 (from.employees))
```

As you can see, the first example with pipeline notation is much easier to
comprehend, compared to the last one with the regular function call notation.
This is why it is recommended to use pipelines for nested function calls that
are 3 or more levels deep.

-->
