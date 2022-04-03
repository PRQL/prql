# Syntax

<!-- Here we could explain how function parameters work, what is a list, S-strings, how to do aliases and so on. -->

### S-Strings

An s-string inserts SQL directly. It's similar in form to a python f-string, but
the result is SQL, rather than a string literal; i.e.:

```prql
func sum col = s"SUM({col})"
sum salary
```

transpiles to:

```sql
SUM(salary)
```

...whereas if it were a python f-string, it would make `"sum(salary)"`, with the
quotes.
