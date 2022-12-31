# Sort

Orders rows based on the values of one or more columns.

```prql_no_test
sort [{direction}{column}]
```

## Parameters

- One column or a list of columns to sort by
- Each column can be prefixed with:
  - `+`, for ascending order, the default
  - `-`, for descending order
- When using prefixes, even a single column needs to be in a list or
  parentheses. (Otherwise, `sort -foo` is parsed as a subtraction between `sort`
  and `foo`.)

## Examples

```prql
from employees
sort age
```

```prql
from employees
sort [-age]
```

```prql
from employees
sort [age, -tenure, +salary]
```

We can also use expressions:

```prql
from employees
sort [s"substr({first_name}, 2, 5)"]
```
