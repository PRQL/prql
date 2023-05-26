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

- within a list (e.g. the `derive` examples below),
- when the following line is a new statement, which starts with a keyword of
  `func`, `let` or `from`.
