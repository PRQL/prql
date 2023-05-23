# Lists

Lists are represented with `{}`, and can span multiple lines. A final trailing
comma is optional.

```prql
from numbers
derive {x = 1, y = 2}
derive {
  a = x,
  b = y
}
derive {
  c = a,
  d = b,
}
```

Most transforms can take either a list or a single item, so these are
equivalent:

```prql
from employees
select {first_name}
```

```prql
from employees
select first_name
```
