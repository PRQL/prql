# Pipes

Pipes are the connection between [transforms](../stdlib/transforms/) that make
up a pipeline. The relation produced by a transform before the pipe is used as
the input for the transform following the pipe. A pipe can be represented with
either a line break or a pipe character (`|`).

For example, here the `filter` transform operates on the result of
`from employees` (which is just the `employees` table), and the `select`
transform operates on the result of the `filter` transform.

```prql
from.employees
filter department == "Product"
select {first_name, last_name}
```

In the place of a line break, it's also possible to use the `|` character to
pipe results between transforms, such that this is equivalent:

```prql
from.employees | filter department == "Product" | select {first_name, last_name}
```

In almost all situations, a line break acts as a pipe. But there are a few
exceptions where a line break doesn't create a pipeline:

- within a tuple
- within an array
- when the following line is a new statement, which starts with a keyword of
  `func`, `let` or `from`
- Within a [line wrap](./operators.md#wrapping-lines)

```prql
[        # Line break OK in an array
  {a=2, b=3}
]
derive {      # Line break OK in a tuple
  c = 2 * a,
}
```

## Inner Transforms

<!-- TODO: I don't think this really fits here -->

Parentheses are also used for transforms (such as `group` and `window`) that
pass their result to an "inner transform". The example below applies the
`aggregate` pipeline to each group of unique `title` and `country` values:

```prql
from.employees
group {title, country} (
  aggregate {
    average salary,
    ct = count salary,
  }
)
```
