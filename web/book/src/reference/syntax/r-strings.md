# R-strings

R-strings handle escape characters without special treatment:

```prql
from artists
derive normal_string =  "\\\t"   #  two characters - \ and tab (\t)
derive raw_string    = r"\\\t"   # four characters - \, \, \, and t
```

An r-string is closed by the same quote character that opened it, so the other
quote character is ordinary content:

```prql
from artists
derive quoted = r"it's"
```
