# Take

Picks rows based on their position.

```prql_no_test
take {n}
```

## Examples

```prql
from employees
take 10
```

## Todo

We could support a range expression so we can get an offset:

```prql_no_test
from employees
take 1..10
```
