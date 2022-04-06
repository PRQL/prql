```elm
from mytable
filter id = $1
```

```sql
SELECT
  mytable.*
FROM
  mytable
WHERE
  id = $1
```
