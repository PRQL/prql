# Syntax

## Summary

A summary of PRQL syntax

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

<!-- I can't seem to get "Quoted identifies" to work without a space between the backticks. VS Code will preview ` `` ` correctly, but not mdbook -->

<!-- TODO: assigns links to select, aliases to join, potentially we should have explicit sections for them?  -->

| Syntax          | Usage                                                                | Example                                                 |
| --------------- | -------------------------------------------------------------------- | ------------------------------------------------------- |
| <code>\|</code> | [Pipelines](queries/pipelines.md)                                    | <code>from employees \| select first_name</code>        |
| `=`             | [Assigns](transforms/select.md) & [Aliases](transforms/join.md)      | `from e = employees` <br> `derive total = (sum salary)` |
| `:`             | [Named args & Parameters](queries/functions.md)                      | `interp lower:0 1600 sat_score`                         |
| `[]`            | [Lists](./syntax.md#lists)                                           | `select [id, amount]`                                   |
| `()`            | [Precedence & Parentheses](./syntax.md#precedence-and-parentheses)   | `derive celsius = (fahrenheit - 32) / 1.8`              |
| `''` & `""`     | [Strings](language-features/strings.md)                              | `derive name = 'Mary'`                                  |
| `` ` ` ``       | [Quoted identifiers](./syntax.md#quoted-identifiers)                 | `` select `first name`  ``                              |
| `#`             | [Comments](./syntax.md#comments)                                     | `# A comment`                                           |
| `@`             | [Dates & Times](language-features/dates-and-times.md#dates--times)   | `@2021-01-01`                                           |
| `==`            | [Expressions](./syntax.md#expressions)                               | `filter a == b and c != d and e > f`                    |
| `==`            | [Self-equality in `join`](transforms/join.md#self-equality-operator) | `join s=salaries [==id]`                                |
| `->`            | [Function definitions](queries/functions.md)                         | `func add a b -> a + b`                                 |
| `+`/`-`         | [Sort order](transforms/sort.md)                                     | `sort [-amount, +date]`                                 |
| `??`            | [Coalesce](language-features/coalesce.md)                            | `amount ?? 0`                                           |

<!--
| `<type>`        | Annotations                                           |  `@2021-01-01<datetime>`                                |
-->

<!-- markdownlint-enable MD033 -->

## Pipes

Pipes — the connection between [transforms](transforms/README.md) that make up a
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

## Expressions

PRQL is made up of _expressions_, like `2 + 3` or `((1 + x) * y)`. In the
example below, note the use of expressions to calculate the alias
`circumference` and in the `filter` transform.

```prql
from foo
select [
  circumference = diameter * 3.14159,
  color,
]
filter circumference > 10 and color != "red"
```

## Precedence and Parentheses

Parentheses — `()` — are used to give _precedence_ to inner expressions.

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
  `sum distance` in
  `round 0 (sum distance)`{{footnote: or, technically, it's on the right
  side of an assignment in a list...}}.
- A minus sign in a function argument, like in `add (-1) (-3)`
- [Inner transforms](#inner-transforms) for `group`, `window`, and other
  transforms.

Parentheses are not required around expressions which use operators but no
function call, like `foo + bar`.

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

For a more formal definition, refer to this precedence table. Because function
call has the lowest precedence, nested function calls or arguments that start or
end with an operator require parenthesis.

| Group          | Operators         | Precedence | Associativity |
| -------------- | ----------------- | ---------- | ------------- |
| identifier dot | `.`               | 1          |               |
| unary          | `- + ! ==`        | 2          |               |
| range          | `..`              | 3          |               |
| mul            | `* / %`           | 4          | left-to-right |
| add            | `+ -`             | 5          | left-to-right |
| compare        | `== != <= >= < >` | 6          | left-to-right |
| coalesce       | `??`              | 7          | left-to-right |
| and            | `and`             | 8          | left-to-right |
| or             | `or`              | 9          | left-to-right |
| function call  |                   | 10         |               |

## Inner Transforms

Parentheses are also used for transforms (such as `group` and `window`) that
pass their result to an "inner transform". The example below applies the
`aggregate` pipeline to each group of unique `title` and `country` values:

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

```prql
from `dir/*.parquet`
```

BigQuery also uses backticks to surround project & dataset names (even if valid
identifiers) in the `SELECT` statement:

```prql
prql target:sql.bigquery
from `project-foo.dataset.table`
join `project-bar.dataset.table` [==col_bax]
```

### Quoting schemas

```note admonish
This is currently not great and we are working on improving it; see
https://github.com/PRQL/prql/issues/1535 for progress.
```

If supplying a schema without a column — for example in a `from` or `join`
transform, that also needs to be a quoted identifier:

```prql
from `music.albums`
```

## Parameters

PRQL will retain parameters like `$1` in SQL output, which can then be supplied
to the SQL query as a prepared query:

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

## Keywords

At the moment, PRQL uses only four keywords:

- `prql`
- `let`
- `func`
- `switch`

If you want to use these names as your columns or relations, use backticks:
`` `switch` ``.

It may seem that transforms are also keywords, but they are normal function
within std namespace:

```
std.from my_table
std.select [from = my_table.a, take = my_table.b]
std.take 3
```

```adonish
Note that new keywords will be added before 1.0 release, so your builds may break.
You can guard against that by using backticks.
```
