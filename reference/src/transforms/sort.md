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

## Examples

```prql
from employees
sort age
```

```prql
from employees
sort -age
```

```prql
from employees
sort [age, -tenure, +salary]
```

## Roadmap

Currently `sort` does not accept expressions:

```prql_no_test
from employees
sort [s"substr({first_name}, 2, 5)"]  # Currently will fail
```
