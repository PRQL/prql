# Inner Transforms

Parentheses are also used for transforms (such as `group` and `window`) that
pass their result to an "inner transform". The example below applies the
`aggregate` pipeline to each group of unique `title` and `country` values:

```prql
from employees
group {title, country} (
  aggregate {
    average salary,
    ct = count salary,
  }
)
```
