# Dates

## Intervals

Intervals are represented by `{N}{periods}`, such as `2years` or `10minutes`,
without a space.

> These aren't the same as ISO8601, because we evaluated `P3Y6M4DT12H30M5S` to
  be difficult to understand, but we could support a simplified form if there's
  demand for it. We don't currently support compound expressions, for example
  `2years10months`, but most DBs will allow `2years + 10months`. Please raise an
  issue if this is inconvenient.

```prql
from projects
derive first_check_in: start + 10days
```
