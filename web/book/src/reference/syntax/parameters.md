# Parameters

Parameter is a placeholder for a value provided after the compilation of the
query.

It uses the following syntax: `$id`, where `id` is a string of letters, digits,
`_` and `.` that doesn't start with a `.`.

Most database engines only support numeric positional parameter ids (i.e `$3`).

```prql
from employees
filter id == $1
```
