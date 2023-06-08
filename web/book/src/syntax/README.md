# Syntax

A summary of PRQL syntax:

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

<!-- I can't seem to get "Quoted identifies" to work without a space between the backticks. VS Code will preview ` `` ` correctly, but not mdbook -->

<!-- TODO: assigns links to select, aliases to join, potentially we should have explicit sections for them?  -->

| Syntax                           | Usage                                                                   | Example                                                 |
| -------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------- |
| <code>\|</code>                  | [Pipelines](../queries/pipelines.md)                                    | <code>from employees \| select first_name</code>        |
| `=`                              | [Assigns](../transforms/select.md) & [Aliases](../transforms/join.md)   | `from e = employees` <br> `derive total = (sum salary)` |
| `:`                              | [Named args & Parameters](../queries/functions.md)                      | `interp low:0 1600 sat_score`                           |
| `{}`                             | [Tuples](./tuples.md)                                                   | `select [id, amount]`                                   |
| <code>! && \|\| == +</code>, etc | [Expressions & Operators](./expressions-and-operators.md)               | <code>filter a == b + c \|\| d >= e</code>              |
| `()`                             | [Parentheses](./expressions-and-operators.md#parentheses)               | `derive celsius = (fahrenheit - 32) / 1.8`              |
| `''`, `""`                       | [Strings](../language-features/strings.md)                              | `derive name = 'Mary'`                                  |
| `` ` ` ``                        | [Quoted identifiers](./quoted-identifiers.md)                           | `` select `first name`  ``                              |
| `#`                              | [Comments](./comments.md)                                               | `# A comment`                                           |
| `@`                              | [Dates & Times](../language-features/dates-and-times.md#dates--times)   | `@2021-01-01`                                           |
| `==`                             | [Self-equality in `join`](../transforms/join.md#self-equality-operator) | `join s=salaries (==id)`                                |
| `->`                             | [Function definitions](../queries/functions.md)                         | `let add = a b -> a + b`                                |
| `=>`                             | [Case statement](../language-features/case.md)                          | `case [a==1 => c, a==2 => d ]`                          |
| `+`/`-`                          | [Sort order](../transforms/sort.md)                                     | `sort [-amount, +date]`                                 |
| `??`                             | [Coalesce](../language-features/coalesce.md)                            | `amount ?? 0`                                           |

<!-- TODO: Arrays -->

<!--
| `<type>`        | Annotations                                           |  `@2021-01-01<datetime>`                                |
-->

<!-- markdownlint-enable MD033 -->
