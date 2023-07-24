# Pipes

Pipes — the connection between [transforms](../transforms/) that make up a
pipeline — can be either line breaks or a pipe character (`|`).

In almost all situations, line-breaks pipe the result of a line's transform into
the transform on the following line. For example, the `filter` transform
operates on the result of `from employees` (which is just the `employees`
table), and the `select` transform operates on the result of the `filter`
transform.

```prql
from employees
filter department == "Product"
select {first_name, last_name}
```

In the place of a line-break, it's also possible to use the `|` character to
pipe results, such that this is equivalent:

```prql
from employees | filter department == "Product" | select {first_name, last_name}
```

A line-break doesn't create a pipeline in a couple of cases:

- within a tuple
- within an array
- when the following line is a new statement, which starts with a keyword of
  `func`, `let` or `from`.

```prql
from [        # Line break OK in an array
  {a=2, b=3}
]
derive {      # Line break OK in a tuple
  c = 2 * a,
}
```

## Inner Transforms

Parentheses are also used for transforms (such as `group` and `window`) that
pass their result to an "inner transform". The example below applies the
`aggregate` pipeline to each group of unique `title` and `country` values:

```prql
from employees
group {title, country} (
  aggregate {
    average salary,
    ct = count salary,
  }
)
```
