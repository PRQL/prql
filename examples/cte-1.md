```elm
table newest_employees = (
from employees
sort tenure
take 50
)

from newest_employees
join salary [id]
select [name, salary]
```

```sql
WITH newest_employees AS (
    SELECT TOP 50 * FROM employees
    ORDER BY tenure
)
SELECT name, salary
FROM newest_employees
JOIN salary USING (id)
```
