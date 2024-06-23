# Syntax

A summary of PRQL syntax:

<!-- markdownlint-disable MD033 — the `|` characters need to be escaped, and surrounded with tags rather than backticks   -->

<!-- I can't seem to get "Quoted identifies" to work without a space between the backticks. VS Code will preview ` `` ` correctly, but not mdbook -->

<!-- TODO: assigns links to select, aliases to join, potentially we should have explicit sections for them?  -->

| Syntax                 | Usage                                                                          | Example                                                 |
| ---------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------- |
| <code>\|</code>        | [Pipelines](./function-calls.md)                                               | <code>from employees \| select first_name</code>        |
| `=`                    | [Assigns](../declarations/variables.md)                                        | `from e = employees` <br> `derive total = (sum salary)` |
| `:`                    | [Named args & parameters](../declarations/functions.md)                        | `interp low:0 1600 sat_score`                           |
| `{}`                   | [Tuples](./tuples.md)                                                          | `{id, false, total = 3}`                                |
| `[]`                   | [Arrays](./arrays.md)                                                          | `[1, 4, 3, 4]`                                          |
| `+`,`!`,`&&`,`==`, etc | [Operators](./operators.md)                                                    | <code>filter a == b + c \|\| d >= e</code>              |
| `()`                   | [Parentheses](./operators.md#parentheses)                                      | `derive celsius = (fht - 32) / 1.8`                     |
| `\`                    | [Line wrap](./operators.md#wrapping-lines)                                     | <code>1 + 2 + 3 +</code><br><code>\ 4 + 5</code>        |
| `1`,`100_000`,`5e10`   | [Numbers](./literals.md#numbers)                                               | `derive { huge = 5e10 * 10_000 }`                       |
| `''`,`""`              | [Strings](./literals.md#strings)                                               | `derive name = 'Mary'`                                  |
| `true`,`false`         | [Booleans](./literals.md#booleans)                                             | `derive { Col1 = true }`                                |
| `null`                 | [Null](./literals.md#null)                                                     | `filter ( name != null )`                               |
| `@`                    | [Dates & times](./literals.md#date-and-time)                                   | `@2021-01-01`                                           |
| `` ` ` ``              | [Quoted identifiers](./keywords.md#quoting)                                    | `` select `first name`  ``                              |
| `#`                    | [Comments](./comments.md)                                                      | `# A comment`                                           |
| `==`                   | [Self-equality in `join`](../stdlib/transforms/join.md#self-equality-operator) | `join s=salaries (==id)`                                |
| `->`                   | [Function definitions](../declarations/functions.md)                           | `let add = a b -> a + b`                                |
| `=>`                   | [Case statement](./case.md)                                                    | `case [a==1 => c, a==2 => d]`                           |
| `+`,`-`                | [Sort order](../stdlib/transforms/sort.md)                                     | `sort {-amount, +date}`                                 |
| `??`                   | [Coalesce](./operators.md#coalesce)                                            | `amount ?? 0`                                           |

<!-- markdownlint-enable MD033 -->
