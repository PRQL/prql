```elm
table newest_employees = (
  from employees
  sort tenure
  take 50
)

table average_salaries = (
  from salaries
  aggregate by:country [
    average_country_salary: average salary
  ]
)

from newest_employees
join average_salaries [country]
select [name, salary, average_country_salary]
```

```sql
WITH newest_employees AS (
  SELECT
    TOP (50) *
  FROM
    employees
  ORDER BY
    tenure
),
average_salaries AS (
  SELECT
    country,
    AVG(salary) AS average_country_salary
  FROM
    salaries
  GROUP BY
    country
)
SELECT
  name,
  salary,
  average_country_salary
FROM
  newest_employees
  JOIN average_salaries USING(country)
```
