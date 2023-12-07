# Compute

Std operators can be used to declare new functions or plain variables

## Examples

```prql
let is_adult = col -> col >= 18
let is_cool = col -> (col | in ["PRQL", "Rust"])

from employees
select {
    first_name,
    last_name,
    hobby,
    adult = is_adult age,
}
filter (is_cool hobby)
```
