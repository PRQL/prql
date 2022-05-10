# Sort

Orders rows based on the values of one or more columns.

```prql_no_test
sort [{direction}{column}]
```

## Arguments

- One or multiple columns
- Each column can be prefixed with:
  - `+`, for ascending order, the default
  - `-`, for descending order

## Examples

```prql
from employees
sort age
```

```prql
from employees
sort (-age)
```

> Note that `sort -age` is not valid; `-age` needs to be surrounded by
> parentheses like `(-age).

```prql
from employees
sort [age, -tenure, +salary]
```
