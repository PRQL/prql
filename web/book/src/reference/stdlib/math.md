# Mathematical functions

These are all the functions defined in the `math` module:

| function | parameters | description                        |
| -------- | ---------- | ---------------------------------- |
| abs      | `col`      | Absolute value of `col`            |
| acos     | `col`      | Arccosine of `col`                 |
| asin     | `col`      | Arcsine of `col`                   |
| atan     | `col`      | Arctangent of `col`                |
| ceil     | `col`      | Rounds the number up of `col`      |
| cos      | `col`      | Cosine of `col`                    |
| degrees  | `col`      | Converts radians to degrees        |
| exp      | `col`      | Exponential of `col`               |
| floor    | `col`      | Rounds the number down             |
| ln       | `col`      | Natural logarithm of `col`         |
| log      | `b` `col`  | `b`-log of `col`                   |
| log10    | `col`      | 10-log of `col`                    |
| pi       |            | The constant π                     |
| pow      | `b` `col`  | Computes `col` to the power `b`    |
| radians  | `col`      | Converts degrees to radians        |
| round    | `n` `col`  | Rounds `col` to `n` decimal places |
| sin      | `col`      | Sin of `col`                       |
| sqrt     | `col`      | Square root of `col`               |
| tan      | `col`      | Tangent of `col`                   |

## Example

```prql
from employees
select age_squared = (age | math.pow 2)
```
