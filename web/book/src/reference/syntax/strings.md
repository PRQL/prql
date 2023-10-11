# Strings

String literals can use any matching odd number of either single or double
quotes:

```prql
from artists
derive {
  single        =   'hello world',
  double        =   "hello world",
  double_triple = """hello world""",
}
```

## Quoting and escape characters

To quote a string containing quote characters, use the "other" type of quote, or
use the escape character `\`, or use more quotes.

```prql
from artists
select {
  other   = '"hello world"',
  escaped = "\"hello world\"",
  triple  = """I said "hello world"!""",
}
```

Strings can contain any escape character sequences defined by the
[JSON standard](https://www.ecma-international.org/publications-and-standards/standards/ecma-404/).

```prql
from artists
derive escapes = "\tXYZ\n \\ "                            # tab (\t), "XYZ", newline (\n), " ", \, " "
derive world = "\u{0048}\u{0065}\u{006C}\u{006C}\u{006F}" # "Hello"
derive hex = "\x48\x65\x6C\x6C\x6F"                       # "Hello"
derive turtle = "\u{01F422}"                              # "🐢"
```

## Other string formats

- [**F-strings**](./f-strings.md) - Build up a new string from a set of columns
  or values.
- [**R-strings**](./r-strings.md) - Include the raw characters of the string
  without any form of escaping.
- [**S-strings**](./s-strings.md) - Insert SQL statements directly into the
  query. Use when PRQL doesn't have an equivalent facility.

```admonish warning
Currently PRQL allows multiline strings with either a single character or
multiple character quotes. This may change for strings using a single character
quote in future versions.
```

```admonish note
These escape rules specify how PRQL interprets escape characters when compiling
strings to SQL, not necessarily how the database will interpret the string.
Dialects interpret escape characters differently,
and PRQL doesn't currently account for
these differences. Please open issues with any difficulties in the current
implementation.
```

## Escape sequences

Unless an `r` prefix is present, escape sequences in string literals are
interpreted according to rules similar to those used by Standard C. The
recognized escape sequences are:

| Escape Sequence | Meaning                       |
| --------------- | ----------------------------- |
| `\\`            | Backslash (\)                 |
| `\'`            | Single quote (')              |
| `\"`            | Double quote (")              |
| `\b`            | Backspace                     |
| `\f`            | Formfeed                      |
| `\n`            | ASCII Linefeed (LF)           |
| `\r`            | ASCII Carriage Return (CR)    |
| `\t`            | ASCII Horizontal Tab (TAB)    |
| `\xhh`          | Character with hex value hh   |
| `\u{xxxx}`      | Character with hex value xxxx |
