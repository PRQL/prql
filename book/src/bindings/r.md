# R (prqlr)

JavaScript bindings for [`prql-compiler`](https://github.com/prql/prql/).
Check out <https://eitsupi.github.io/prqlr/> for more context.

## Installation

`install.packages("prqlr", repos = "https://eitsupi.r-universe.dev")`

## Usage

```r
library(prqlr)

"
from employees
join salaries [emp_id]
group [dept_id, gender] (
  aggregate [
    avg_salary = average salary
  ]
)
" |>
  prql_to_sql()
```
