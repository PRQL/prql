# R-strings

R-strings handle escape characters without special treatment:

```prql
from db.artists
derive normal_string =  "\\\t"   #  two characters - \ and tab (\t)
derive raw_string    = r"\\\t"   # four characters - \, \, \, and t
```
