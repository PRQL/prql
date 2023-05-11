# Sort

Orders rows based on the values of one or more columns.

```prql no-eval
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

## Ordering guarantees

PRQL will persist orderings through transforms where possible. Most DBs don't
natively do this for operations such as `JOIN`. For example in:

```prql
from employees
sort tenure
join locations [==employee_id]
```

...PRQL compiles the `ORDER BY` to the _end_ of the query.
