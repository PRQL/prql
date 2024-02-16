# Tuples

Tuple is a container type, composed of multiple fields. Each field can have a
different type. Number of fields and their types must be known at compile time.

Tuple is represented by `{}`. It can span multiple lines. Fields can be assigned
a name. Fields are separated by commas, trailing trailing comma is optional.

```prql no-eval
let var1 = {x = 1, y = 2}

let var2 = {           # Span multiple lines
  a = x,
  b = y                # Optional trailing comma
}

let var3 = {
  c,                   # Individual item
  d = b,               # Assignment
}
```

Tuples are the type of a table row, which means that they are expected by many
transforms. Most transforms can also take a single field, which will be
converted into a tuple. These are equivalent:

```prql
from db.employees
select {first_name}
```

```prql
from db.employees
select first_name
```

```admonish note
Prior to `0.9.0`, tuples were previously named Lists, and represented with
`[]` syntax. There may still be references to the old naming.
```
