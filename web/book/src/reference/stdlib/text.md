# Text functions

These are all the functions defined in the `text` module:

| function    | parameters             | description                                                                   |
| ----------- | ---------------------- | ----------------------------------------------------------------------------- |
| contains    | `sub` `col`            | Returns true if `col` contains `sub`                                          |
| ends_with   | `sub` `col`            | Returns true if `col` ends with `sub`                                         |
| extract     | `idx` `len` `col`      | Extracts a substring at the index `idx` (starting at 1) with the length `len` |
| length      | `col`                  | Returns the number of characters in `col`                                     |
| lower       | `col`                  | Converts `col` to lower case                                                  |
| ltrim       | `col`                  | Removes all the whitespaces from the left side of `col`                       |
| replace     | `before` `after` `col` | Replaces any occurrences of `before` with `after` in `col`                    |
| rtrim       | `col`                  | Removes all the whitespaces from the right side of `col`                      |
| starts_with | `sub` `col`            | Returns true if `col` starts with `sub`                                       |
| trim        | `col`                  | Removes all the whitespaces from both sides of `col`                          |
| upper       | `col`                  | Converts `col` to upper case                                                  |

## Pattern matching

`contains`, `starts_with` and `ends_with` compile to SQL `LIKE`, and their
argument becomes the `LIKE` pattern rather than a literal string. `%` and `_` in
the argument therefore keep their wildcard meaning — `text.contains "a_b"`
matches `axb` as well as `a_b` — and PRQL does not escape them. Case sensitivity
follows the target database's `LIKE` semantics, so lower-case the column and use
a lower-case pattern for matching that is case-insensitive everywhere, as in the
example below.

## Example

```prql
from employees
select {
  (last_name | text.lower | text.starts_with("a")),
  (title | text.replace "manager" "chief"),
}
```
