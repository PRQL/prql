# Syntax

## Summary

A summary of PRQL syntax

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

<!-- I can't seem to get "Quoted identifies" to work without a space between the backticks. VSCode will preview ` `` ` correctly, but not mdbook -->

<!-- TODO: assigns links to select, aliases to join, potentially we should have explicit sections for them?  -->

| Syntax          | Usage                                                                   | Example                                                 |
| --------------- | ----------------------------------------------------------------------- | ------------------------------------------------------- |
| <code>\|</code> | [Pipelines](./pipelines.md)                                             | <code>from employees \| select first_name</code>        |
| `=`             | [Assigns](../transforms/select.md) & [Aliases](../transforms/join.md)   | `from e = employees` <br> `derive total = (sum salary)` |
| `:`             | [Named args & Parameters](./functions.md)                               | `interp lower:0 1600 sat_score`                         |
| `[]`            | [Lists](./syntax.md#lists)                                              | `select [id, amount]`                                   |
| `()`            | [Precedence](./syntax.md#parentheses)                                   | `derive celsius = (fahrenheit - 32) / 1.8`              |
| `''` & `""`     | [Strings](../language-features/strings.md)                              | `derive name = 'Mary'`                                  |
| `` ` ` ``       | [Quoted identifiers](./syntax.md#quoted-identifiers)                    | `` select `first name`  ``                              |
| `#`             | [Comments](./syntax.md#comments)                                        | `# A comment`                                           |
| `@`             | [Dates & Times](../language-features/dates-and-times.md#dates--times)   | `@2021-01-01`                                           |
| `==`            | Equality                                                                | `filter [a == b, c != d, e > f]`                        |
| `==`            | [Self-equality in `join`](../transforms/join.md#self-equality-operator) | `join s=salaries [==id]`                                |
| `->`            | [Function definitions](./functions.md)                                  | `func add a b -> a + b`                                 |
| `+`/`-`         | [Sort order](../transforms/sort.md)                                     | `sort [-amount, +date]`                                 |
| `??`            | [Coalesce](../language-features/coalesce.md)                            | `amount ?? 0`                                           |

<!--
| `<type>`        | Annotations                                           |  `@2021-01-01<datetime>`                                |
-->

<!-- markdownlint-enable MD033 -->

## Pipes

Pipes — the connection between [transforms](../transforms.md) that make up a
pipeline — can be either line breaks or a pipe character (`|`).

In almost all situations, line-breaks pipe the result of a line's transform into
the transform on the following line. For example, the `filter` transform
operates on the result of `from employees` (which is just the `employees`
table), and the `select` transform operates on the result of the `filter`
transform.

```prql
from employees
filter department == "Product"
select [first_name, last_name]
```

In the place of a line-break, it's also possible to use the `|` character to
pipe results, such that this is equivalent:

```prql
from employees | filter department == "Product" | select [first_name, last_name]
```

A line-break doesn't create a pipeline in a couple of cases:

- within a list (e.g. the `derive` examples below),
- when the following line is a new statement, which starts with a keyword of
  `func`, `table` or `from`.

## Lists

Lists are represented with `[]`, and can span multiple lines. A final trailing
comma is optional.

```prql
from numbers
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

Most transforms can take either a list or a single item, so these are
equivalent:

```prql
from employees
select [first_name]
```

```prql
from employees
select first_name
```

## Parentheses

<!-- TODO document conclusions from https://github.com/PRQL/prql/issues/648  -->

Parentheses — `()` — are used to give precedence to inner expressions, like in
other languages.

Parentheses are required around:

- any expression containing a pipe, either the `|` symbol or a linebreak
- any function call that isn't otherwise separated from other expressions (for
  example, it's in a list)
- when a minus sign is used to make a negative value in a function argument

```prql
from employees
# Requires parentheses, because it's contains a pipe
derive is_proximate = (distance | in 0..20)
# Requires parentheses, because it's a function call
derive average_distance = (sum distance)
derive [
  # Requires parentheses, because it's contains a pipe
  is_mid = (distance | in 20..100),
  # Doesn't require parentheses, because it's in a list!
  is_far = in 100.. distance,
  # The left value of the range also requires parentheses,
  # because of the minus sign
  is_negative = (distance | in (-100..0))
]
# Requires parentheses because of the minus sign
sort (-distance)
```

```admonish note
We realize some of this is not intuitive. We are considering approaches to
make this more intuitive — even at the cost of requiring more syntax in some
circumstances. And we're planning to make the error messages much better,
so the compiler s there to help out.
```

Consistent with this, in transforms which take another transform as an argument,
such as `group` and `window`, parentheses are used to nest the inner transforms.
Here, the `aggregate` pipeline is applied to each group of unique `title` and
`country` values:

```prql
from employees
group [title, country] (
  aggregate [
    average salary,
    ct = count
  ]
)
```

## Comments

Comments are represented by `#`.

```prql
from employees  # Comment 1
# Comment 2
aggregate [average salary]
```

There's no distinct multiline comment syntax.

## Quoted identifiers

To use identifiers that are otherwise invalid, surround them with backticks.
Depending on the dialect, these will remain as backticks or be converted to
double-quotes.

```prql
prql target:sql.mysql
from employees
select `first name`
```

```prql
prql target:sql.postgres
from employees
select `first name`
```

BigQuery also uses backticks to surround project & dataset names (even if valid
identifiers) in the `SELECT` statement:

```prql
prql target:sql.bigquery
from `project-foo.dataset.table`
join `project-bar.dataset.table` [==col_bax]
```

## Parameters

PRQL will retain parameters like `$1` in SQL output, which can then be supplied
to the SQL query:

```prql
from employees
filter id == $1
```

## Numbers

Numbers can contain underscores between numbers; which can make reading large
numbers easier:

```prql
from numbers
select [
    small = 1.000_000_1,
    big = 5_000_000,
]
```
