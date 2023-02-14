# F-Strings

F-strings are a readable approach to building new strings from existing strings.
Currently PRQL supports this for concatenating strings:

```prql
from employees
select full_name = f"{first_name} {last_name}"
```

This can be much easier to read for longer strings, relative to the SQL
approach:

```prql
from web
select url = f"http{tls}://www.{domain}.{tld}/{page}"
```

Note that interpolations can only contain plain variable names and not whole
expression like Python.

## Roadmap

In the future, f-strings may incorporate string formatting such as datetimes,
numbers, and padding. If there's a feature that would be helpful, please
[post an issue](https://github.com/PRQL/prql/issues/new/).
