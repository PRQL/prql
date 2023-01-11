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

Parentheses — `()` — are used to give precedence to inner expressions.

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
  `sum distance` in `round 0 (sum distance)`[^1].
- A minus sign in a function argument, like in `add (-1) (-3)`

Parentheses are not required around expressions which use operators but no
function call, like `foo + bar`.

[^1]: or, technically, on the right side of an assignment in a list.

Here's a full rundown of times this applier:

```prql
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
  # Doesn't require parentheses, because it's in a list (confusing, see footnote)!
  average_distance = average distance,
]
# Requires parentheses because of the minus sign
sort (-distance)
# A list is fine too
sort [-distance]
```

Consistent with this, parentheses are used to nest the inner transforms in
transforms which take another transform as an argument, such as `group` and
`window`, Here, the `aggregate` pipeline is applied to each group of unique
`title` and `country` values:

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
