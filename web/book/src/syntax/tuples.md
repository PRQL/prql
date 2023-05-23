# Tuples

Tuples are represented with `{}`. They can span multiple lines. They can contain
both individual items and assignments. A final trailing comma is optional.

```prql
from numbers
derive {x = 1, y = 2}
derive {               # Span multiple lines
  a = x,
  b = y                # Optional trailing comma
}
select {
  c,                   # Individual item
  d = b,               # Assignment
}
```

Most transforms can take single item or a tuple, so these are equivalent:

```prql
from employees
select {first_name}
```

```prql
from employees
select first_name
```

```admonish note
Prior to `0.9.0`, tuples were previously named Lists, and represented with `[]` syntax. There may still be references to the old naming.
```
