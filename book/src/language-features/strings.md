# Strings

Strings in PRQL can use either single or double quotes:

```prql
from my_table
select x = "hello world"
```

```prql
from my_table
select x = 'hello world'
```

To quote a string containing quotes, either use the "other" type of quote, or
use three-or-more quotes, and close with the same number.

```prql
from my_table
select x = '"hello world"'
```

```prql
from my_table
select x = """I said "hello world"!"""
```

```prql
from my_table
select x = """""I said """hello world"""!"""""
```

## F-Strings and S-Strings

These special case strings can be used to:

[F-Strings](./f-strings.md) - Build up a new string from a set of columns or
values

[S-Strings](./s-strings.md) - Insert SQL statements directly into the query. Use
when PRQL doesn't have an equivalent facility.

```admonish note
Currently PRQL does not adjust escape characters.
```

```admonish warning
Currently PRQL allows multiline strings with either a single character or
multiple character quotes. This may change for strings using a single character
quote in future versions.
```
