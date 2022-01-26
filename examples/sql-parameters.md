```elm
from mytable
filter id = $1    # We should be able to pass parameters straight through.
```

```sql
SELECT * FROM mytable WHERE id = $1
```
