# Dates

### Intervals

It's possible to represent intervals with a single expression:

```prql
from projects
derive [
  first_check_in: start + 10days
]
```
