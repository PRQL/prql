# F-Strings

f-strings are a readable approach to building new strings from existing strings.
Currently PRQL supports this for concatenating strings:

```prql
from x
select [
  full_name: f"{first} {last}"
]
```

This can be much easier to read for longer strings, relative to the SQL approach:

```prql
from x
select [
  url: f"http{tls}://www.{domain}.{tld}/{page}"
]
```

In the future, this may extend to other types of formatting, such as datetimes,
numbers, and padding. If there's a feature that would be helpful, please [post
an issue](https://github.com/prql/prql/issues/new/choose).
