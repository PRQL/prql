# Syntax

## Summary

A summary of PRQL syntax

<!-- The `|` characters need to be escaped, and surrounded with tags rather than backticks -->

| Syntax          | Usage                   | Example                                                 |
| --------------- | ----------------------- | ------------------------------------------------------- |
| <code>\|</code> | Pipe                    | <code>from employees \| select first_name</code>        |
| `=`             | Assigns & Aliases       | `from e = employees` <br> `derive total = (sum salary)` |
| `:`             | Named args & Parameters | `interp lower:0 1600 sat_score`                         |
| `[]`            | List                    | `select [id, amount]`                                   |
| `()`            | Precedence              | `derive fahrenheit = (celsius - 32) * 1.8`              |
| `#`             | Comment                 | `# A comment`                                           |
| `@`             | Date & times            | `@2021-01-01`                                           |
| `==`            | Equality comparison     | `join s=salaries [s.emp_id == e.id]`                    |
| `->`            | Function definitions    | `func add a b -> a + b`                                 |
| `+`/`-`         | Sort order              | `sort [-amount, +date]`                                 |
| `??`            | Coalesce                | `amount ?? 0`                                           |
| `<type>`        | Annotations             | `@2021-01-01<datetime>`                                 |

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

## Backticks

To use identifiers that are otherwise invalid, surround them with backticks.
Depending on the dialect, these will remain as backticks or be converted to
double-quotes

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
join `project-bar.dataset.table` [col_bax]
```
