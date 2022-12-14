# Syntax

## Summary

A summary of PRQL syntax

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

<!-- I can't seem to get "Quoted identifies" to work without a space between the backticks. VSCode will preview ` `` ` correctly, but not mdbook -->
<!-- prettier-ignore-start -->
| Syntax          | Usage                                                        | Example                                                 |
| --------------- | ------------------------------------------------------------ | ------------------------------------------------------- |
| <code>\|</code> | [Pipelines](/queries/pipelines.md)                           | <code>from employees \| select first_name</code>        |
| `=`             | Assigns & Aliases                                            | `from e = employees` <br> `derive total = (sum salary)` |
| `:`             | [Named args & Parameters](/queries/functions.md)             | `interp lower:0 1600 sat_score`                         |
| `[]`            | [Lists](/queries/syntax.md#lists)                            | `select [id, amount]`                                   |
| `()`            | [Precedence and Parentheses](/queries/syntax.md#parentheses) | `derive fahrenheit = celsius * 1.8 + 32`                |
| `''`, `""` or `"""..."""` | [Strings](../language-features/strings.md)         | `derive name = "Mary"` _or_ <br> ``derive quotes = """`"'"""`` |
| `` ` ` ``       | [Quoted identifiers](/queries/syntax.md#quoted-identifiers)  | `` select `first name`  ``                              |
| `#`             | [Comments](/queries/syntax.md#comments)                      | `# A comment`                                           |
| `@`             | [Dates & Times](../language-features/dates_and_times.md#dates--times) | `@2021-01-01`                                  |
| `==`            | Comparisons                                                  | `filter [a == b, c != d, e > f]`                        |
| `==`            | [Self-equality in join](../transforms/join.md#self-equality-operator) | `join s=salaries [s.emp_id == e.id]` _or_  <br> `join s=salaries [==id]` |
| `->`            | [Function definitions](/queries/functions.md)                | `func add a b -> a + b`                                 |
| `+`/`-`         | [Sort order](../transforms/sort.md)                          | `sort [-amount, +date]`                                 |
| `??`            | [Coalesce](../language-features/coalesce.md)                 | `amount ?? 0`                                           |
| `<type>`        | Annotations                                                  | _Are\_these\_available?_ _Maybe link to ../language-features/dates_and_times.md_ `@2021-01-01<datetime>` |
<!-- prettier-ignore-end -->

<!-- markdownlint-enable MD033 -->

## Pipes

Pipes — the connection between [transforms](../transforms.md) that make up a
pipeline — can be either line breaks or a pipe character (`|`).

In almost all situations, line-breaks pipe the result of a line's transform into the transform on
the following line. For example, the `filter` transform operates on the result
of `from employees` (which is just the `employees` table), and the `select` transform operates on
the result of the `filter` transform.

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

Parentheses — `()` — are used to give precedence to inner expressions, as is the
case in almost all languages / math.

In particular, parentheses are used to nest pipelines for transforms such as
`group` and `window`, which take a pipeline. Here, the `aggregate` pipeline is
applied to each group of unique `title` and `country` values.

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

Comments are represented by `#`. Currently only single line comments exist.

```prql
from employees  # Comment 1
# Comment 2
aggregate [average salary]
```

## Quoted identifiers

To use identifiers that are otherwise invalid, surround them with backticks.
Depending on the dialect, these will remain as backticks or be converted to
double-quotes.

```prql
prql dialect:mysql
from employees
select `first name`
```

```prql
prql dialect:postgres
from employees
select `first name`
```

BigQuery also uses backticks to surround project & dataset names (even if valid
identifiers) in the `SELECT` statement:

```prql
prql dialect:bigquery
from `project-foo.dataset.table`
join `project-bar.dataset.table` [==col_bax]
```
