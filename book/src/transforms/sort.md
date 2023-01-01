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

## Notes

### Ordering guarantees

Most DBs will persist through ordering most transforms; for example, you can
expect this result to be ordered by `tenure`.

```prql
from employees
sort tenure
derive name = f"{first_name} {last_name}"
```

But:

- This is an implementation detail of the DB. If there are instances where this
  doesn't hold, please open an issue, and we'll consider how to manage it.
- Some transforms which change the existence of rows, such as `join` or `group`,
  won't persist ordering; for example:

```prql
from employees
sort tenure
join locations [==employee_id]
```

See [Issue #1363](https://github.com/PRQL/prql/issues/1363) for more details.
