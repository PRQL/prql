```prql
let newest_employees = (
  from employees
  sort tenure
  take 50
)

let average_salaries = (
  from salaries
  group country (
    aggregate average_country_salary = (average salary)
  )
)

from newest_employees
join average_salaries (==country)
select {name, salary, average_country_salary}
```
