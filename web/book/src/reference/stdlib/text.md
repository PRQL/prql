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

### Example

```prql
from employees
select {
  last_name | text.lower | text.starts_with("a"),
  title | text.replace "manager" "chief"
}
```
