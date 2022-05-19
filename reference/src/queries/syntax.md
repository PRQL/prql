# Syntax

## Line-breaks & `|` character

A line-break generally pipes the result of that line into the transformation on
the following line. For example, the `filter` and `select` transform operates on
the result of the previous line:

```prql
from employees
filter department == "Product"
select [first_name, last_name]
```

In the place of a line-break, it's also possible to use the `|` character to
pipe results:

```prql
from employees | filter department == "Product" | select [first_name, last_name]
```

A line-break doesn't created a pipeline in a couple of cases:

- Within a list (e.g. the `derive` examples below).
- When the following line is a new statement, by starting with a keyword such as
  `func`.

## Lists

Lists are represented with `[]`, and can span multiple lines. A final trailing
comma is optional.

```prql
derive [x = 1, y = 2]
derive [
  a = x,
  b = y
]
derive [
  c = a,
  d = b,
]
```

## Syntax summary

A summary of PRQL syntax
<!-- We need to double escape the `|` characters because one of them is removed by cmark; ref https://github.com/prql/prql/issues/514. -->
| Syntax           | Usage                   | Example                                                     |
| ---------------- | ----------------------- | ----------------------------------------------------------- |
| <code>\\|</code> | Pipe                    | <code>from employees \\| select first_name</code>           |
| `=`              | Assigns & Aliases       | `derive temp_c = (c_of_f temp_f)` <br> `from e = employees` |
| `:`              | Named args & Parameters | `interp lower:0 1600 sat_score`                             |
| `==`             | Equality comparison     | `join s=salaries [s.employee_id == employees.id]`           |
| `->`             | Function definitions    | `func add a b -> a + b`                                     |
| `+`/`-`          | Sort order              | `sort [-amount, +date]`                                     |
| `??`             | Coalesce                | `amount ?? 0`                                               |
| `<type>`         | Annotations             | `@2021-01-01<datetime>`                                     |
