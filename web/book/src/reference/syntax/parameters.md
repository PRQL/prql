# Parameters

Parameter is a placeholder for a value provided after the compilation of the
query.

It uses the following syntax: `$id`, where `id` is an arbitrary alpha numeric
string.

Most database engines only support numeric positional parameter ids (i.e `$3`).

```prql
from db.employees
filter id == $1
```
